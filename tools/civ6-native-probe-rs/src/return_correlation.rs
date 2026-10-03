//! Candidate return values are attached to an observed call frame, never to
//! an address alone. An exception can unwind that frame without a return.

const MAX_RETURN_SITES: usize = 2;
const MAX_PENDING_CALLS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntryKind {
    CanEver,
    CanAppoint,
}

impl EntryKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::CanEver => "can_ever",
            Self::CanAppoint => "can_appoint",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EntryInput {
    pub id: u64,
    pub entry: EntryKind,
    pub return_address: u64,
    pub expected_rsp: u64,
    pub this_pointer: u64,
    pub governor_index: u32,
    pub direct_call_verified: bool,
    pub return_site_valid: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingCall {
    pub id: u64,
    pub entry: EntryKind,
    pub return_address: u64,
    pub expected_rsp: u64,
    pub this_pointer: u64,
    pub governor_index: u32,
    pub direct_call_verified: bool,
    pub linked_parent: Option<u64>,
    pub eligible: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Incomplete {
    pub id: u64,
    pub reason: &'static str,
}

#[derive(Debug)]
pub(crate) struct EnterDecision {
    pub incomplete: Vec<Incomplete>,
    pub linked_parent: Option<u64>,
    pub armed: bool,
    pub unarmed_reason: Option<&'static str>,
}

#[derive(Debug)]
pub(crate) struct ReturnDecision {
    pub complete: Vec<PendingCall>,
    pub incomplete: Vec<Incomplete>,
    pub linked_tail_group: Option<u64>,
    pub unmatched: bool,
}

#[derive(Default)]
pub(crate) struct ReturnCorrelation {
    pending: Vec<PendingCall>,
}

impl ReturnCorrelation {
    pub(crate) fn has_site(&self, address: u64) -> bool {
        self.pending.iter().any(|p| p.return_address == address)
    }

    pub(crate) fn sites(&self) -> Vec<u64> {
        let mut sites = Vec::new();
        for call in self.pending.iter().rev() {
            if !sites.contains(&call.return_address) {
                sites.push(call.return_address);
            }
        }
        sites
    }

    /// A new entry at the same or shallower stack frame supersedes an old
    /// pending call. Only a verified CanAppoint -> CanEver tail can retain the
    /// parent at the exact same return address and stack pointer.
    pub(crate) fn enter(&mut self, input: EntryInput) -> EnterDecision {
        let parent = self
            .pending
            .last()
            .filter(|p| {
                input.entry == EntryKind::CanEver
                    && input.return_site_valid
                    && !input.direct_call_verified
                    && p.entry == EntryKind::CanAppoint
                    && p.direct_call_verified
                    && p.eligible
                    && p.return_address == input.return_address
                    && p.expected_rsp == input.expected_rsp
                    && p.this_pointer == input.this_pointer
                    && p.governor_index == input.governor_index
            })
            .map(|p| p.id);

        let mut incomplete = Vec::new();
        self.pending.retain(|p| {
            let keep = p.eligible && (Some(p.id) == parent || p.expected_rsp > input.expected_rsp);
            if !keep && p.eligible {
                incomplete.push(Incomplete {
                    id: p.id,
                    reason: "frame_superseded_before_return",
                });
            }
            keep
        });

        let unarmed_reason = if !input.return_site_valid {
            Some("return_site_unavailable")
        } else if self.pending.len() >= MAX_PENDING_CALLS {
            Some("pending_call_limit")
        } else if !self.has_site(input.return_address) && self.sites().len() >= MAX_RETURN_SITES {
            Some("return_site_limit")
        } else {
            None
        };
        if let Some(reason) = unarmed_reason {
            if let Some(parent_id) = parent {
                self.pending.retain(|p| {
                    if p.id == parent_id {
                        incomplete.push(Incomplete {
                            id: p.id,
                            reason: "tail_child_unarmed",
                        });
                        false
                    } else {
                        true
                    }
                });
            }
            return EnterDecision {
                incomplete,
                linked_parent: None,
                armed: false,
                unarmed_reason: Some(reason),
            };
        }
        self.pending.push(PendingCall {
            id: input.id,
            entry: input.entry,
            return_address: input.return_address,
            expected_rsp: input.expected_rsp,
            this_pointer: input.this_pointer,
            governor_index: input.governor_index,
            direct_call_verified: input.direct_call_verified,
            linked_parent: parent,
            eligible: true,
        });
        EnterDecision {
            incomplete,
            linked_parent: parent,
            armed: true,
            unarmed_reason: None,
        }
    }

    /// Passing an application exception to its handler can unwind an active
    /// call. Preserve the breakpoint until the next safe rearm, but never use
    /// a later AL value for these IDs.
    pub(crate) fn invalidate_for_exception(&mut self) -> Vec<Incomplete> {
        self.pending
            .iter_mut()
            .filter_map(|p| {
                if p.eligible {
                    p.eligible = false;
                    Some(Incomplete {
                        id: p.id,
                        reason: "exception_may_unwind_frame",
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    pub(crate) fn returned(&mut self, address: u64, rsp: u64) -> ReturnDecision {
        let mut incomplete = Vec::new();
        self.pending.retain(|p| {
            let keep = p.expected_rsp >= rsp;
            if !keep && p.eligible {
                incomplete.push(Incomplete {
                    id: p.id,
                    reason: "frame_unwound_before_return",
                });
            }
            keep
        });
        let mut matched = Vec::new();
        self.pending.retain(|p| {
            if p.return_address == address && p.expected_rsp == rsp {
                matched.push(p.clone());
                false
            } else {
                true
            }
        });
        if matched.is_empty() {
            return ReturnDecision {
                complete: Vec::new(),
                incomplete,
                linked_tail_group: None,
                unmatched: true,
            };
        }
        let pair_parent = if matched.len() == 2 && matched.iter().all(|p| p.eligible) {
            let parent = matched.iter().find(|p| p.entry == EntryKind::CanAppoint);
            let child = matched.iter().find(|p| p.entry == EntryKind::CanEver);
            match (parent, child) {
                (Some(parent), Some(child))
                    if parent.direct_call_verified
                        && child.linked_parent == Some(parent.id)
                        && parent.this_pointer == child.this_pointer
                        && parent.governor_index == child.governor_index =>
                {
                    Some(parent.id)
                }
                _ => None,
            }
        } else {
            None
        };
        let single =
            matched.len() == 1 && matched[0].eligible && matched[0].linked_parent.is_none();
        if single || pair_parent.is_some() {
            return ReturnDecision {
                complete: matched,
                incomplete,
                linked_tail_group: pair_parent,
                unmatched: false,
            };
        }
        for call in matched {
            if call.eligible {
                incomplete.push(Incomplete {
                    id: call.id,
                    reason: "unverified_overlapping_return",
                });
            }
        }
        ReturnDecision {
            complete: Vec::new(),
            incomplete,
            linked_tail_group: None,
            unmatched: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(id: u64, entry: EntryKind, address: u64, rsp: u64) -> EntryInput {
        EntryInput {
            id,
            entry,
            return_address: address,
            expected_rsp: rsp,
            this_pointer: 0x1234,
            governor_index: 7,
            direct_call_verified: true,
            return_site_valid: true,
        }
    }

    #[test]
    fn same_stack_reuse_after_nonlocal_exit_never_inherits_the_old_result() {
        let mut calls = ReturnCorrelation::default();
        assert!(
            calls
                .enter(input(1, EntryKind::CanEver, 0x2000, 0x3000))
                .armed
        );
        assert_eq!(
            calls.invalidate_for_exception(),
            vec![Incomplete {
                id: 1,
                reason: "exception_may_unwind_frame"
            }]
        );
        assert!(
            calls
                .enter(input(2, EntryKind::CanEver, 0x2000, 0x3000))
                .armed
        );
        let result = calls.returned(0x2000, 0x3000);
        assert_eq!(
            result.complete.iter().map(|p| p.id).collect::<Vec<_>>(),
            [2]
        );
        assert!(!result.unmatched);

        assert!(
            calls
                .enter(input(3, EntryKind::CanEver, 0x2000, 0x3000))
                .armed
        );
        let next = calls.enter(input(4, EntryKind::CanEver, 0x2000, 0x3000));
        assert_eq!(
            next.incomplete,
            [Incomplete {
                id: 3,
                reason: "frame_superseded_before_return"
            }]
        );
        assert_eq!(calls.returned(0x2000, 0x3000).complete[0].id, 4);

        calls.enter(input(5, EntryKind::CanEver, 0x2000, 0x3000));
        assert_eq!(calls.invalidate_for_exception().len(), 1);
        assert!(calls.returned(0x2000, 0x3000).complete.is_empty());
    }

    #[test]
    fn verified_tail_links_only_the_same_parent_frame_and_identity() {
        let mut calls = ReturnCorrelation::default();
        calls.enter(input(10, EntryKind::CanAppoint, 0x305b, 0x4000));
        let mut child = input(11, EntryKind::CanEver, 0x305b, 0x4000);
        child.direct_call_verified = false;
        assert_eq!(calls.enter(child).linked_parent, Some(10));
        let result = calls.returned(0x305b, 0x4000);
        assert_eq!(result.linked_tail_group, Some(10));
        assert_eq!(
            result.complete.iter().map(|p| p.id).collect::<Vec<_>>(),
            [10, 11]
        );

        calls.enter(input(12, EntryKind::CanAppoint, 0x305b, 0x4000));
        child.id = 13;
        child.this_pointer = 0x9999;
        let decision = calls.enter(child);
        assert_eq!(decision.linked_parent, None);
        assert_eq!(
            decision.incomplete,
            [Incomplete {
                id: 12,
                reason: "frame_superseded_before_return"
            }]
        );
        assert_eq!(calls.returned(0x305b, 0x4000).complete[0].id, 13);

        let mut unverified = input(14, EntryKind::CanAppoint, 0x305b, 0x4000);
        unverified.direct_call_verified = false;
        calls.enter(unverified);
        child.id = 15;
        child.this_pointer = 0x1234;
        assert_eq!(calls.enter(child).linked_parent, None);
        assert_eq!(calls.returned(0x305b, 0x4000).complete[0].id, 15);
    }

    #[test]
    fn unrelated_matching_candidates_never_receive_the_same_al() {
        let mut calls = ReturnCorrelation::default();
        calls.enter(input(1, EntryKind::CanEver, 0x2000, 0x3000));
        let mut unrelated = calls.pending[0].clone();
        unrelated.id = 2;
        unrelated.linked_parent = None;
        calls.pending.push(unrelated);
        let result = calls.returned(0x2000, 0x3000);
        assert!(result.complete.is_empty());
        assert_eq!(
            result.incomplete,
            [
                Incomplete {
                    id: 1,
                    reason: "unverified_overlapping_return"
                },
                Incomplete {
                    id: 2,
                    reason: "unverified_overlapping_return"
                },
            ]
        );
        assert!(!calls.has_site(0x2000));

        calls.enter(input(3, EntryKind::CanEver, 0x2000, 0x3000));
        let wrong_stack = calls.returned(0x2000, 0x2000);
        assert!(wrong_stack.complete.is_empty());
        assert!(wrong_stack.unmatched);
        let unwound = calls.returned(0x2000, 0x4000);
        assert!(unwound.complete.is_empty());
        assert_eq!(
            unwound.incomplete,
            [Incomplete {
                id: 3,
                reason: "frame_unwound_before_return"
            }]
        );
    }

    #[test]
    fn nested_frames_keep_the_outer_result_but_site_and_count_limits_hold() {
        let mut calls = ReturnCorrelation::default();
        calls.enter(input(1, EntryKind::CanEver, 0x2000, 0x4000));
        calls.enter(input(2, EntryKind::CanEver, 0x2100, 0x3000));
        assert_eq!(
            calls
                .enter(input(3, EntryKind::CanEver, 0x2200, 0x2000))
                .unarmed_reason,
            Some("return_site_limit")
        );
        assert_eq!(calls.returned(0x2100, 0x3000).complete[0].id, 2);
        assert_eq!(calls.returned(0x2000, 0x4000).complete[0].id, 1);

        for id in 1..=MAX_PENDING_CALLS as u64 {
            assert!(
                calls
                    .enter(input(id, EntryKind::CanEver, 0x2000, 0x10000 - id * 16))
                    .armed
            );
        }
        assert_eq!(
            calls
                .enter(input(99, EntryKind::CanEver, 0x2000, 0x1000))
                .unarmed_reason,
            Some("pending_call_limit")
        );
    }
}
