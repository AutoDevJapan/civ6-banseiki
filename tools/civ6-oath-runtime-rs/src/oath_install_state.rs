//! Portable install-state model for the Oath observer.
//!
//! The native installer writes game memory in steps that can each fail half
//! way. Every decision about what may still be undone lives here, as a pure
//! function of what is known, so it can be checked over all outcome orders:
//!
//! * Reach is a one-way latch. It rises when the prefix is written (or when a
//!   write may have happened), because from that moment any writer of the
//!   entry, ours or foreign, could branch into the prefix, even for an
//!   instant. Nothing lowers it: not a second look at the entry, not a
//!   rollback. The padding is therefore never recycled once written.
//! * A binding (call type -> target, trampoline, trace) is Held only while
//!   Reach is Never, and Frozen for good once it is Possibly. Only a verified
//!   release of a Never binding, whose memory was never written, returns it to
//!   Free.
//! * A region is settled only when its bytes, its page protection and its
//!   instruction cache flush are all confirmed. Bytes alone never settle it.

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Reach {
    /// No byte of ours was ever written; nothing could have branched in.
    Never,
    /// Something might have; never lowered again.
    Possibly,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Binding {
    Free,
    Held,
    Frozen,
}

/// What is known about the bytes of a region.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Bytes {
    Original,
    Ours,
    /// A write was attempted and its outcome is not verified.
    Unknown,
    /// Verified to be neither the original nor ours.
    Foreign,
}

/// Protection and cache flush after the last operation on a region.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Confirm {
    pub protection: bool,
    pub flush: bool,
}

impl Confirm {
    pub const OK: Self = Self {
        protection: true,
        flush: true,
    };
    pub const DIRTY: Self = Self {
        protection: false,
        flush: false,
    };
    pub fn complete(self) -> bool {
        self.protection && self.flush
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Region {
    pub bytes: Bytes,
    pub confirm: Confirm,
}

impl Region {
    pub const START: Self = Self {
        bytes: Bytes::Original,
        confirm: Confirm::OK,
    };
    pub fn settled_original(self) -> bool {
        self.bytes == Bytes::Original && self.confirm.complete()
    }
}

/// Outcome of a guarded byte write.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Write {
    /// The page could not be made writable; nothing changed.
    Refused,
    /// The bytes were not the expected ones; they were left alone.
    Mismatch { restored: bool },
    /// Bytes untouched, but the protection could not be put back.
    ProtectionUnverified,
    Written {
        restored: bool,
        flushed: bool,
        verified: bool,
    },
}

/// Outcome of the two byte compare-exchange on the entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cas {
    /// The page could not be made writable; the exchange never ran.
    Untouched,
    /// Another writer's bytes were in the entry.
    Lost { restored: bool },
    Done {
        restored: bool,
        flushed: bool,
        verified: bool,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Ledger {
    reach: Reach,
    binding: Binding,
    pub prefix: Region,
    pub entry: Region,
}

impl Default for Ledger {
    fn default() -> Self {
        Self::new()
    }
}

impl Ledger {
    pub const fn new() -> Self {
        Self {
            reach: Reach::Never,
            binding: Binding::Free,
            prefix: Region::START,
            entry: Region::START,
        }
    }
    pub fn reach(&self) -> Reach {
        self.reach
    }
    pub fn binding(&self) -> Binding {
        self.binding
    }
    pub fn frozen(&self) -> bool {
        self.binding == Binding::Frozen
    }

    fn raise(&mut self) {
        self.reach = Reach::Possibly;
        if self.binding == Binding::Held {
            self.binding = Binding::Frozen;
        }
    }

    /// Takes the call type for one target. Only a Free binding can be taken.
    pub fn bind(&mut self) -> bool {
        if self.binding == Binding::Free && self.reach == Reach::Never {
            self.binding = Binding::Held;
            true
        } else {
            false
        }
    }

    /// Call before the prefix write; returns what to restore on a refusal.
    pub fn prefix_begin(&mut self) -> Region {
        let previous = self.prefix;
        self.prefix = Region {
            bytes: Bytes::Unknown,
            confirm: Confirm::DIRTY,
        };
        previous
    }
    /// Call right before the bytes are copied (after the page is writable and
    /// the expected bytes matched): the prefix may be ours from here on.
    pub fn prefix_copy_begin(&mut self) {
        self.raise();
    }
    pub fn prefix_end(&mut self, previous: Region, target: Bytes, result: Write) {
        if matches!(result, Write::Written { .. }) {
            self.raise();
        }
        self.prefix = after_write(previous, target, result);
    }

    /// Call right after the entry became writable and before the exchange
    /// runs: from here on the entry may hold bytes that reach the prefix.
    pub fn entry_begin(&mut self) -> Region {
        self.raise();
        let previous = self.entry;
        self.entry = Region {
            bytes: Bytes::Unknown,
            confirm: Confirm::DIRTY,
        };
        previous
    }
    pub fn entry_end(&mut self, previous: Region, target: Bytes, result: Cas) {
        self.entry = match result {
            Cas::Untouched => previous,
            Cas::Lost { restored } => Region {
                bytes: Bytes::Foreign,
                confirm: Confirm {
                    protection: restored,
                    flush: previous.confirm.flush,
                },
            },
            Cas::Done {
                restored,
                flushed,
                verified,
            } => Region {
                bytes: if verified { target } else { Bytes::Unknown },
                confirm: Confirm {
                    protection: restored,
                    flush: flushed,
                },
            },
        };
    }

    /// Facts read back from memory. A prefix that holds our jump, or an entry
    /// that is not the original while the prefix is not the padding, could
    /// branch into the prefix; a second look at the bytes never lowers that.
    pub fn observe(&mut self, prefix: Bytes, entry: Bytes) {
        debug_assert!(prefix != Bytes::Unknown && entry != Bytes::Unknown);
        self.prefix.bytes = prefix;
        self.entry.bytes = entry;
        if prefix == Bytes::Ours
            || entry == Bytes::Ours
            || (entry != Bytes::Original && prefix != Bytes::Original)
        {
            self.raise();
        }
    }
    pub fn prefix_repaired(&mut self, confirm: Confirm) {
        self.prefix.confirm = confirm;
    }
    pub fn entry_repaired(&mut self, confirm: Confirm) {
        self.entry.confirm = confirm;
    }

    pub fn may_restore_entry(&self) -> bool {
        self.entry.bytes == Bytes::Ours
    }
    /// Reservations (trace claim, trampoline cell, target binding) go back
    /// only when no byte of ours was ever written (Reach is Never) and the
    /// pages are confirmed as they were. Foreign bytes in the padding are not
    /// ours to undo; they simply mean this install was refused.
    pub fn may_release(&self) -> bool {
        self.binding == Binding::Held
            && self.reach == Reach::Never
            && self.prefix.bytes != Bytes::Ours
            && self.prefix.bytes != Bytes::Unknown
            && self.prefix.confirm.complete()
            && self.entry.settled_original()
    }
    /// Frees a Never binding and forgets its regions for the next install.
    pub fn release(&mut self) -> bool {
        if self.may_release() {
            *self = Self::new();
            true
        } else {
            false
        }
    }
    /// Nothing of ours is left to restore: the entry is original or foreign,
    /// with protection and flush confirmed (the prefix too).
    pub fn settled(&self) -> bool {
        matches!(self.entry.bytes, Bytes::Original | Bytes::Foreign)
            && self.entry.confirm.complete()
            && self.prefix.confirm.complete()
    }
}

/// The trace is one object shared by both call types. It is needed while
/// any call type holds a binding (Held, or Frozen for good) and only then; who
/// claimed it first does not matter.
pub fn trace_needed<'a>(ledgers: impl IntoIterator<Item = &'a Ledger>) -> bool {
    ledgers
        .into_iter()
        .any(|ledger| ledger.binding() != Binding::Free)
}

fn after_write(previous: Region, target: Bytes, result: Write) -> Region {
    match result {
        Write::Refused => previous,
        Write::Mismatch { restored } => Region {
            bytes: Bytes::Foreign,
            confirm: Confirm {
                protection: restored,
                flush: previous.confirm.flush,
            },
        },
        Write::ProtectionUnverified => Region {
            bytes: previous.bytes,
            confirm: Confirm {
                protection: false,
                flush: previous.confirm.flush,
            },
        },
        Write::Written {
            restored,
            flushed,
            verified,
        } => Region {
            bytes: if verified { target } else { Bytes::Unknown },
            confirm: Confirm {
                protection: restored,
                flush: flushed,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    const BOOLS: [bool; 2] = [false, true];

    fn writes() -> Vec<Write> {
        let mut all = vec![Write::Refused, Write::ProtectionUnverified];
        all.extend(BOOLS.map(|restored| Write::Mismatch { restored }));
        for restored in BOOLS {
            for flushed in BOOLS {
                for verified in BOOLS {
                    all.push(Write::Written {
                        restored,
                        flushed,
                        verified,
                    });
                }
            }
        }
        all
    }
    fn cases() -> Vec<Cas> {
        let mut all = vec![];
        all.extend(BOOLS.map(|restored| Cas::Lost { restored }));
        for restored in BOOLS {
            for flushed in BOOLS {
                for verified in BOOLS {
                    all.push(Cas::Done {
                        restored,
                        flushed,
                        verified,
                    });
                }
            }
        }
        all
    }
    fn confirms() -> Vec<Confirm> {
        BOOLS
            .into_iter()
            .flat_map(|protection| BOOLS.map(move |flush| Confirm { protection, flush }))
            .collect()
    }

    /// Every operation a correct driver may issue next from a state.
    fn successors(state: Ledger) -> Vec<(Ledger, bool)> {
        let mut out = vec![];
        let mut push = |next: Ledger, released: bool| out.push((next, released));
        if state.binding() == Binding::Free {
            let mut next = state;
            if next.bind() {
                push(next, false);
            }
            return out;
        }
        for result in writes() {
            for target in [Bytes::Ours, Bytes::Original] {
                let mut next = state;
                let previous = next.prefix_begin();
                if matches!(result, Write::Written { .. }) {
                    next.prefix_copy_begin();
                }
                next.prefix_end(previous, target, result);
                push(next, false);
            }
        }
        for result in cases() {
            let mut next = state;
            let previous = next.entry_begin();
            next.entry_end(previous, Bytes::Ours, result);
            push(next, false);
            if state.may_restore_entry() {
                let mut next = state;
                let previous = next.entry_begin();
                next.entry_end(previous, Bytes::Original, result);
                push(next, false);
            }
        }
        for prefix in [Bytes::Original, Bytes::Ours, Bytes::Foreign] {
            for entry in [Bytes::Original, Bytes::Ours, Bytes::Foreign] {
                let mut next = state;
                next.observe(prefix, entry);
                push(next, false);
            }
        }
        for confirm in confirms() {
            let mut next = state;
            next.prefix_repaired(confirm);
            push(next, false);
            let mut next = state;
            next.entry_repaired(confirm);
            push(next, false);
        }
        let mut next = state;
        if next.release() {
            push(next, true);
        }
        out
    }

    fn reachable() -> Vec<Ledger> {
        let mut seen: HashSet<Ledger> = HashSet::new();
        let mut queue = vec![Ledger::new()];
        while let Some(state) = queue.pop() {
            if !seen.insert(state) {
                continue;
            }
            for (next, _) in successors(state) {
                if !seen.contains(&next) {
                    queue.push(next);
                }
            }
        }
        seen.into_iter().collect()
    }

    #[test]
    fn invariants_hold_over_every_outcome_order() {
        let states = reachable();
        assert!(states.len() > 300, "explored {}", states.len());
        for state in states {
            // Reach and the binding agree; written bytes always latch.
            if state.reach() == Reach::Possibly {
                assert_eq!(state.binding(), Binding::Frozen, "{state:?}");
                assert!(!state.may_release(), "{state:?}");
            }
            if state.prefix.bytes == Bytes::Ours || state.entry.bytes == Bytes::Ours {
                assert_eq!(state.reach(), Reach::Possibly, "{state:?}");
            }
            if state.may_release() {
                assert_eq!(state.reach(), Reach::Never);
                assert!(state.prefix.confirm.complete() && state.entry.settled_original());
            }
            for (next, released) in successors(state) {
                // The latch only rises.
                assert!(!(state.reach() == Reach::Possibly && next.reach() == Reach::Never));
                // Frozen is forever.
                if state.frozen() {
                    assert!(next.frozen(), "{state:?} -> {next:?}");
                }
                // A binding returns to Free only through a verified release.
                if state.binding() != Binding::Free && next.binding() == Binding::Free {
                    assert!(released && state.may_release(), "{state:?} -> {next:?}");
                    assert_eq!(next, Ledger::new());
                }
            }
        }
    }

    #[test]
    fn writing_the_prefix_latches_reach_before_anything_else_can_happen() {
        let mut ledger = Ledger::new();
        assert!(ledger.bind());
        let previous = ledger.prefix_begin();
        // The entry of a foreign writer could branch in right after this.
        ledger.prefix_copy_begin();
        assert_eq!(ledger.reach(), Reach::Possibly);
        assert!(ledger.frozen());
        ledger.prefix_end(
            previous,
            Bytes::Ours,
            Write::Written {
                restored: true,
                flushed: true,
                verified: true,
            },
        );
        // A foreign branch into the prefix and back to the original bytes
        // (ABA) is invisible to a later look; the latch stays.
        ledger.observe(Bytes::Ours, Bytes::Original);
        assert_eq!(ledger.reach(), Reach::Possibly);
        assert!(!ledger.may_release());
        let previous = ledger.entry_begin();
        ledger.entry_end(previous, Bytes::Ours, Cas::Lost { restored: true });
        assert_eq!(ledger.entry.bytes, Bytes::Foreign);
        assert!(ledger.settled() || !ledger.prefix.confirm.complete());
    }

    #[test]
    fn nothing_written_releases_only_when_protection_and_flush_are_confirmed() {
        let mut ledger = Ledger::new();
        assert!(ledger.bind());
        let previous = ledger.prefix_begin();
        ledger.prefix_end(previous, Bytes::Ours, Write::ProtectionUnverified);
        assert_eq!(ledger.reach(), Reach::Never, "no byte was written");
        assert_eq!(ledger.prefix.bytes, Bytes::Original);
        assert!(!ledger.may_release(), "protection is unconfirmed");
        ledger.prefix_repaired(Confirm {
            protection: true,
            flush: false,
        });
        assert!(!ledger.may_release(), "flush is unconfirmed");
        ledger.prefix_repaired(Confirm::OK);
        assert!(ledger.release());
        assert_eq!(ledger, Ledger::new());
        assert!(ledger.bind(), "a verified release frees the binding");
    }

    #[test]
    fn a_refused_or_mismatched_write_never_latches() {
        let mut ledger = Ledger::new();
        assert!(ledger.bind());
        let previous = ledger.prefix_begin();
        ledger.prefix_end(previous, Bytes::Ours, Write::Refused);
        assert_eq!(
            (ledger.prefix, ledger.reach()),
            (Region::START, Reach::Never)
        );
        let previous = ledger.prefix_begin();
        ledger.prefix_end(previous, Bytes::Ours, Write::Mismatch { restored: true });
        assert_eq!(ledger.prefix.bytes, Bytes::Foreign);
        assert_eq!(ledger.reach(), Reach::Never);
        assert!(
            ledger.release(),
            "foreign padding: refused, not ours to undo"
        );
    }

    #[test]
    fn the_shared_trace_is_needed_exactly_while_any_binding_is() {
        let states = reachable();
        for a in &states {
            for b in &states {
                let held = a.binding() != Binding::Free || b.binding() != Binding::Free;
                assert_eq!(trace_needed([a, b]), held);
                // A frozen binding keeps the trace whatever the other one does.
                if a.frozen() {
                    assert!(trace_needed([a, &Ledger::new()]));
                }
                // Releasing one kind leaves the trace to the other kind.
                let mut released = *a;
                if released.release() {
                    assert_eq!(trace_needed([&released, b]), b.binding() != Binding::Free);
                }
            }
        }
    }
}
