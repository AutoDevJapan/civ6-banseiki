//! Invocation-local identity observations for the pinned XP2 City adapters.
//! No pointer is retained as a lifetime or payment/quantity authority. The
//! memory provider supplies readable copies; this does not establish its lock
//! coverage, definition names, target unit filters, or Apply completion.

use crate::{Error, Result};

pub trait Memory {
    fn copy(&mut self, address: u64, bytes: &mut [u8]) -> Result<()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Apply,
    Remove,
}

#[derive(Clone, Copy, Debug)]
/// Capture every field at the +28/+30 City adapter's ENTRY, from the same
/// invocation: original caller return, RCX effect, R8D ID and RBP/RDI. Inside
/// the adapter R8 is overwritten, so later values are not this frame contract.
pub struct Frame {
    pub module_base: u64,
    pub call: Call,
    pub caller_return: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub effect: u64,
    pub instance_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityObservation {
    pub instance: u64,
    pub instance_id: u32,
    pub definition: u64,
    pub effect_vtable_rva: u64,
    /// The original effect holds the integer percentage at +8. It is converted
    /// to Q8 by Apply, so a stored Amount=15 is NOT the Q8 contribution=38.
    pub declared_percent: i32,
}

fn address(base: u64, offset: u64) -> Result<u64> {
    base.checked_add(offset)
        .ok_or_else(|| Error::input("oath identity address overflow"))
}

fn pointer(value: u64) -> Result<u64> {
    if value == 0 || value & 7 != 0 {
        return Err(Error::input("null or unaligned oath identity pointer"));
    }
    Ok(value)
}

fn read<const N: usize>(memory: &mut impl Memory, address: u64) -> Result<[u8; N]> {
    let mut data = [0; N];
    memory.copy(address, &mut data)?;
    Ok(data)
}

fn observe(memory: &mut impl Memory, frame: Frame) -> Result<IdentityObservation> {
    let caller = frame
        .caller_return
        .checked_sub(frame.module_base)
        .ok_or_else(|| Error::input("oath caller outside module"))?;
    let instance = pointer(match (frame.call, caller) {
        (Call::Apply, 0x944e55 | 0x944f27) => frame.rbp,
        (Call::Remove, 0x974287) => frame.rdi,
        _ => return Err(Error::input("unknown oath adapter caller")),
    })?;
    let effect = pointer(frame.effect)?;
    if u64::from_le_bytes(read(memory, instance)?) != address(frame.module_base, 0xaa3e38)? {
        return Err(Error::input("unknown oath Instance vtable"));
    }
    if u32::from_le_bytes(read(memory, address(instance, 0xe8)?)?) != frame.instance_id {
        return Err(Error::input("oath caller and Instance ID differ"));
    }
    if u64::from_le_bytes(read(memory, address(instance, 0x1b8)?)?) != effect {
        return Err(Error::input("oath caller and effect receiver differ"));
    }
    let definition = pointer(u64::from_le_bytes(read(memory, address(instance, 0x168)?)?))?;
    let effect_vtable = u64::from_le_bytes(read(memory, effect)?);
    let rva = effect_vtable
        .checked_sub(frame.module_base)
        .ok_or_else(|| Error::input("oath effect vtable outside module"))?;
    let pair = match rva {
        0xa76a30 => (0x817810, 0x8188a0),
        0xa6cdb8 => (0x7e8560, 0x7e8db0),
        0xa90328 => (0x8947a0, 0x895460),
        0xa9e0f0 => (0x8d8d60, 0x8d9790),
        _ => return Err(Error::input("unknown oath effect family")),
    };
    for (slot, target) in [
        (0x28, 0x8a75e0),
        (0x30, 0x8a8440),
        (0x60, pair.0),
        (0x68, pair.1),
    ] {
        if u64::from_le_bytes(read(memory, address(effect_vtable, slot)?)?)
            != address(frame.module_base, target)?
        {
            return Err(Error::input("oath effect dispatch differs"));
        }
    }
    Ok(IdentityObservation {
        instance,
        instance_id: frame.instance_id,
        definition,
        effect_vtable_rva: rva,
        declared_percent: i32::from_le_bytes(read(memory, address(effect, 8)?)?),
    })
}

/// Two readable copies must agree. Agreement is an observation only: an ABA
/// change or publication before Apply cannot be ruled out by rereading.
/// Unreadable or unsupported frames return errors, never a zero contribution.
pub fn copy_identity(memory: &mut impl Memory, frame: Frame) -> Result<IdentityObservation> {
    let first = observe(memory, frame)?;
    if observe(memory, frame)? != first {
        return Err(Error::input("oath call identity changed while copying"));
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Fixture {
        bytes: BTreeMap<u64, u8>,
        changed: bool,
        change_amount: bool,
        count: usize,
    }
    impl Fixture {
        fn put(&mut self, at: u64, bytes: &[u8]) {
            self.bytes
                .extend(bytes.iter().enumerate().map(|(i, b)| (at + i as u64, *b)));
        }
        fn u64(&mut self, at: u64, value: u64) {
            self.put(at, &value.to_le_bytes());
        }
    }
    impl Memory for Fixture {
        fn copy(&mut self, at: u64, data: &mut [u8]) -> Result<()> {
            self.count += 1;
            if self.changed && self.count == 11 {
                self.put(0x2000 + 0xe8, &465u32.to_le_bytes());
            }
            if self.change_amount && self.count == 11 {
                self.put(0x4008, &16i32.to_le_bytes());
            }
            for (i, b) in data.iter_mut().enumerate() {
                *b = *self
                    .bytes
                    .get(&(at + i as u64))
                    .ok_or_else(|| Error::input("fixture unreadable"))?;
            }
            Ok(())
        }
    }
    fn fixture(base: u64) -> (Fixture, Frame) {
        let mut memory = Fixture {
            bytes: BTreeMap::new(),
            changed: false,
            change_amount: false,
            count: 0,
        };
        memory.u64(0x2000, base + 0xaa3e38);
        memory.put(0x2000 + 0xe8, &464u32.to_le_bytes());
        memory.u64(0x2000 + 0x1b8, 0x4000);
        memory.u64(0x2000 + 0x168, 0x6000);
        memory.u64(0x4000, base + 0xa76a30);
        memory.put(0x4008, &15i32.to_le_bytes());
        for (slot, target) in [
            (0x28, 0x8a75e0),
            (0x30, 0x8a8440),
            (0x60, 0x817810),
            (0x68, 0x8188a0),
        ] {
            memory.u64(base + 0xa76a30 + slot, base + target);
        }
        (
            memory,
            Frame {
                module_base: base,
                call: Call::Apply,
                caller_return: base + 0x944e55,
                rbp: 0x2000,
                rdi: 0,
                effect: 0x4000,
                instance_id: 464,
            },
        )
    }

    #[test]
    fn all_known_caller_frames_use_the_correct_nonvolatile_receiver() {
        for (call, caller) in [
            (Call::Apply, 0x944e55),
            (Call::Apply, 0x944f27),
            (Call::Remove, 0x974287),
        ] {
            for base in [0x180000000, 0x7ffb12340000] {
                let (mut memory, mut frame) = fixture(base);
                frame.call = call;
                frame.caller_return = base + caller;
                if call == Call::Remove {
                    frame.rdi = frame.rbp;
                    frame.rbp = 0;
                }
                let observed = copy_identity(&mut memory, frame).unwrap();
                assert_eq!(observed.instance_id, 464);
                assert_eq!(observed.definition, 0x6000);
                assert_eq!(observed.declared_percent, 15);
            }
        }
    }

    #[test]
    fn wrong_frame_id_receiver_family_and_null_definition_are_unknown() {
        for case in 0..6 {
            let (mut memory, mut frame) = fixture(0x180000000);
            match case {
                0 => frame.caller_return += 1,
                1 => frame.call = Call::Remove,
                2 => frame.instance_id += 1,
                3 => frame.effect += 8,
                4 => memory.u64(0x4000, frame.module_base + 0xa76aa0),
                5 => memory.u64(0x2168, 0),
                _ => unreachable!(),
            }
            assert!(copy_identity(&mut memory, frame).is_err());
        }
    }

    #[test]
    fn stale_or_incomplete_copies_never_mean_zero_contribution() {
        let (mut memory, frame) = fixture(0x180000000);
        memory.changed = true;
        assert!(copy_identity(&mut memory, frame).is_err());
        let (mut memory, frame) = fixture(0x180000000);
        memory.bytes.remove(&0x4009);
        assert!(copy_identity(&mut memory, frame).is_err());
        let (mut memory, frame) = fixture(0x180000000);
        memory.u64(
            frame.module_base + 0xa76a30 + 0x60,
            frame.module_base + 0x8188a0,
        );
        assert!(copy_identity(&mut memory, frame).is_err());
    }

    #[test]
    fn two_valid_but_different_observations_are_rejected() {
        let (mut memory, frame) = fixture(0x180000000);
        memory.change_amount = true;
        let error = copy_identity(&mut memory, frame).unwrap_err();
        assert!(error.to_string().contains("changed while copying"));
    }
}
