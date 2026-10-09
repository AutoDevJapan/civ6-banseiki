//! Invocation-local identity observations for the pinned XP2 City adapters.
//! No pointer is retained as a lifetime or payment/quantity authority. The
//! memory provider supplies readable copies; this does not establish its lock
//! coverage, definition names, target unit filters, or Apply completion.

/// Allocation-free errors for bounded reads inside a native callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error(pub &'static str);
impl Error {
    pub fn input(message: &'static str) -> Self {
        Self(message)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(self.0)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

pub trait Memory {
    fn copy(&mut self, address: u64, bytes: &mut [u8]) -> Result<()>;
}

/// Copies from the actual callee's RDX and original caller registers, without
/// invoking the component getter (which performs COW/map operations).
/// It relates this invocation to its current backpointer only; no lifetime,
/// ABA exclusion, contribution amount or later pointer validity is inferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecipientIdentity {
    pub instance: u64,
    pub instance_id: u32,
    pub definition: u64,
    pub declared_percent: i32,
    pub city_wrapper: u64,
    pub owner: u16,
    pub full_id: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct RecipientFrame {
    pub base: u64,
    pub slot: usize,
    pub caller: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub effect: u64,
    pub actual_receiver: u64,
}

pub fn copy_recipient_identity(
    memory: &mut impl Memory,
    frame: RecipientFrame,
) -> Result<RecipientIdentity> {
    let RecipientFrame {
        base,
        slot,
        caller,
        rbp,
        rdi,
        effect,
        actual_receiver,
    } = frame;
    let observe = |memory: &mut dyn Memory| -> Result<RecipientIdentity> {
        // Each adapter tail dispatch preserves the original return and RBP/RDI.
        let offset = caller
            .checked_sub(base)
            .ok_or_else(|| Error::input("recipient caller outside image"))?;
        let instance = pointer(match (slot % 2, offset) {
            (0, 0x944e55 | 0x944f27) => rbp,
            (1, 0x974287) => rdi,
            _ => return Err(Error::input("unsupported recipient caller")),
        })?;
        let table = *[0xa76a30u64, 0xa6cdb8, 0xa90328, 0xa9e0f0]
            .get(slot / 2)
            .ok_or_else(|| Error::input("invalid recipient family"))?;
        let effect = pointer(effect)?;
        if u64::from_le_bytes(read(memory, instance)?) != address(base, 0xaa3e38)?
            || u64::from_le_bytes(read(memory, address(instance, 0x1b8)?)?) != effect
            || u64::from_le_bytes(read(memory, effect)?) != address(base, table)?
        {
            return Err(Error::input("recipient instance or effect differs"));
        }
        let definition = pointer(u64::from_le_bytes(read(memory, address(instance, 0x168)?)?))?;
        let city = pointer(actual_receiver)?;
        let wrapper = read_pointer(memory, address(city, 0x1968)?)?;
        let indicated = observe_city(memory, base, wrapper)?;
        Ok(RecipientIdentity {
            instance,
            instance_id: u32::from_le_bytes(read(memory, address(instance, 0xe8)?)?),
            definition,
            declared_percent: i32::from_le_bytes(read(memory, address(effect, 8)?)?),
            city_wrapper: wrapper,
            owner: indicated.owner,
            full_id: indicated.full_id,
        })
    };
    let first = observe(memory)?;
    if observe(memory)? != first {
        return Err(Error::input("recipient identity changed during copy"));
    }
    Ok(first)
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

fn read<const N: usize>(memory: &mut (impl Memory + ?Sized), address: u64) -> Result<[u8; N]> {
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

/// Identity indicated by a City wrapper. This is not proof of the City that
/// the adapter subsequently resolves, its lifetime, or its current owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndicatedCity {
    pub owner: u16,
    pub full_id: u32,
}
fn observe_city(
    memory: &mut (impl Memory + ?Sized),
    module_base: u64,
    object: u64,
) -> Result<IndicatedCity> {
    let object = pointer(object)?;
    if u64::from_le_bytes(read(memory, object)?) != address(module_base, 0xab4f58)? {
        return Err(Error::input("unknown City wrapper vtable"));
    }
    let identity = read::<4>(memory, address(object, 0x138)?)?;
    let owner = u16::from_le_bytes(identity[..2].try_into().unwrap());
    let kind = u16::from_le_bytes(identity[2..].try_into().unwrap());
    if kind != 2 || owner >= 64 || u64::from_le_bytes(read(memory, address(object, 0x140)?)?) != 0 {
        return Err(Error::input("unsupported City wrapper identity"));
    }
    Ok(IndicatedCity {
        owner,
        full_id: u32::from_le_bytes(read(memory, address(object, 0x13c)?)?),
    })
}
pub fn copy_indicated_city(
    memory: &mut impl Memory,
    module_base: u64,
    object: u64,
) -> Result<IndicatedCity> {
    let first = observe_city(memory, module_base, object)?;
    if observe_city(memory, module_base, object)? != first {
        return Err(Error::input("City wrapper identity changed while copying"));
    }
    Ok(first)
}

/// A readable copy of the native low-16-bit table lookup. This is a candidate
/// only: neither full generation equality, the callee's actual receiver, nor
/// the candidate's lifetime is established by this copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CityTableObservation {
    pub wrapper_object: u64,
    pub owner: u16,
    pub full_id: u32,
    pub candidate_city: u64,
    pub game_state: u64,
    pub player: u64,
    pub table: u64,
    pub group: u64,
}

fn read_pointer(memory: &mut (impl Memory + ?Sized), at: u64) -> Result<u64> {
    pointer(u64::from_le_bytes(read(memory, at)?))
}
fn city_table_observation(
    memory: &mut impl Memory,
    base: u64,
    object: u64,
) -> Result<CityTableObservation> {
    let indicated = observe_city(memory, base, object)?;
    if indicated.full_id == u32::MAX {
        return Err(Error::input("invalid City table id"));
    }
    let global = read_pointer(memory, address(base, 0xb8aa68)?)?;
    let game_state = read_pointer(memory, address(global, 0x240)?)?;
    let owners = read_pointer(memory, address(game_state, 0x2b48)?)?;
    if i32::from_le_bytes(read(
        memory,
        address(owners, u64::from(indicated.owner) * 4)?,
    )?) == -1
    {
        return Err(Error::input("inactive City table owner"));
    }
    let players = read_pointer(memory, address(game_state, 0x20)?)?;
    let player = read_pointer(memory, address(players, u64::from(indicated.owner) * 8)?)?;
    let table = read_pointer(memory, address(player, 0x6d0)?)?;
    let begin = read_pointer(memory, address(table, 0x98)?)?;
    let end = u64::from_le_bytes(read(memory, address(table, 0xa0)?)?);
    let len = end
        .checked_sub(begin)
        .ok_or_else(|| Error::input("reversed City table range"))?;
    if len % 8 != 0 {
        return Err(Error::input("unaligned City table range"));
    }
    let slot = u64::from(indicated.full_id & 0xffff);
    let group_index = slot >> 3;
    if group_index >= len / 8 {
        return Err(Error::input("City table slot outside range"));
    }
    let group = read_pointer(memory, address(begin, group_index * 8)?)?;
    let candidate_city = read_pointer(memory, address(group, (slot & 7) * 24)?)?;
    Ok(CityTableObservation {
        wrapper_object: object,
        owner: indicated.owner,
        full_id: indicated.full_id,
        candidate_city,
        game_state,
        player,
        table,
        group,
    })
}

pub fn copy_city_table_observation(
    memory: &mut impl Memory,
    base: u64,
    object: u64,
) -> Result<CityTableObservation> {
    let first = city_table_observation(memory, base, object)?;
    if city_table_observation(memory, base, object)? != first {
        return Err(Error::input("City table graph changed while copying"));
    }
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Clone)]
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
    #[test]
    fn indicated_city_requires_the_whole_known_wrapper_and_never_resolves_a_city() {
        let (mut memory, frame) = fixture(0x10000000);
        let object = 0x6000;
        memory.u64(object, frame.module_base + 0xab4f58);
        memory.put(object + 0x138, &[0, 0, 2, 0]);
        memory.put(object + 0x13c, &65536u32.to_le_bytes());
        memory.u64(object + 0x140, 0);
        assert_eq!(
            copy_indicated_city(&mut memory, frame.module_base, object).unwrap(),
            IndicatedCity {
                owner: 0,
                full_id: 65536
            }
        );
        memory.put(object + 0x13a, &1u16.to_le_bytes());
        assert!(copy_indicated_city(&mut memory, frame.module_base, object).is_err());
        memory.put(object + 0x13a, &2u16.to_le_bytes());
        memory.put(object + 0x144, &1u32.to_le_bytes());
        assert!(copy_indicated_city(&mut memory, frame.module_base, object).is_err());
        memory.u64(object + 0x140, 0);
        memory.u64(object, frame.module_base + 0xab4f60);
        assert!(copy_indicated_city(&mut memory, frame.module_base, object).is_err());
        assert!(copy_indicated_city(&mut memory, frame.module_base, 0).is_err());
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
    fn actual_callee_copy_uses_rdx_backpointer_and_never_r8_as_instance_id() {
        let base = 0x180000000;
        let (mut memory, frame) = fixture(base);
        let city = 0x10000;
        let wrapper = 0x8000;
        memory.u64(city + 0x1968, wrapper);
        memory.u64(wrapper, base + 0xab4f58);
        memory.put(wrapper + 0x138, &0u16.to_le_bytes());
        memory.put(wrapper + 0x13a, &2u16.to_le_bytes());
        memory.put(wrapper + 0x13c, &65536u32.to_le_bytes());
        memory.u64(wrapper + 0x140, 0);
        let read = |memory: &mut Fixture, slot, receiver| {
            copy_recipient_identity(
                memory,
                RecipientFrame {
                    base,
                    slot,
                    caller: frame.caller_return,
                    rbp: frame.rbp,
                    rdi: 0,
                    effect: frame.effect,
                    actual_receiver: receiver,
                },
            )
        };
        let identity = read(&mut memory, 0, city).unwrap();
        assert_eq!(
            (
                identity.instance_id,
                identity.declared_percent,
                identity.owner,
                identity.full_id
            ),
            (464, 15, 0, 65536)
        );
        assert_eq!(identity.city_wrapper, wrapper);
        assert!(read(&mut memory, 2, city).is_err());
        assert!(read(&mut memory, 0, city + 8).is_err());
        assert!(read(&mut memory, 1, city).is_err());
        memory.u64(city + 0x1968, 0);
        assert!(read(&mut memory, 0, city).is_err());
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

    #[test]
    fn city_table_observation_returns_subset_and_not_full_resolution() {
        let mut memory = Fixture {
            bytes: BTreeMap::new(),
            changed: false,
            change_amount: false,
            count: 0,
        };
        let base = 0x180000000;
        let object = 0x6000;

        // Wrapper object setup
        memory.u64(object, base + 0xab4f58);
        memory.put(object + 0x138, &[2, 0, 2, 0]); // owner 2, kind 2
        let full_id = 0x10005u32; // slot 5
        memory.put(object + 0x13c, &full_id.to_le_bytes());
        memory.u64(object + 0x140, 0);

        // Global state setup
        memory.u64(base + 0xb8aa68, 0x1000);
        memory.u64(0x1000 + 0x240, 0x2000); // global state
        memory.u64(0x2000 + 0x2b48, 0x3000); // active owner array
        memory.put(0x3000 + 2 * 4, &0i32.to_le_bytes()); // active_owner[2] = 0
        memory.u64(0x2000 + 0x20, 0x4000); // player pointer array
        memory.u64(0x4000 + 2 * 8, 0x5000); // player = 5000
        memory.u64(0x5000 + 0x6d0, 0x6000); // city table = 6000

        // City table setup (len > slot_group 0)
        memory.u64(0x6000 + 0x98, 0x7000); // begin
        memory.u64(0x6000 + 0xa0, 0x7008); // end (len = 1 element of 8 bytes)
        memory.u64(0x7000, 0x8000); // group 0 ptr

        // Slot 5 is in group 0 (since 5 >> 3 == 0)
        // Group index is 5. Slot offset is 5 * 3 = 15. offset in bytes = 15 * 8 = 120 (0x78)
        let candidate = 0x9000;
        memory.u64(0x8000 + 120, candidate);

        let obs = copy_city_table_observation(&mut memory, base, object).unwrap();
        assert_eq!(obs.owner, 2);
        assert_eq!(obs.full_id, full_id);
        assert_eq!(obs.wrapper_object, object);
        assert_eq!(obs.candidate_city, candidate);
        let mut reversed = memory.clone();
        reversed.u64(0x6000 + 0xa0, 0x6ff8);
        assert!(copy_city_table_observation(&mut reversed, base, object).is_err());
        let mut outside = memory.clone();
        outside.u64(0x6000 + 0xa0, 0x7000);
        assert!(copy_city_table_observation(&mut outside, base, object).is_err());
        let mut partial = memory.clone();
        partial.bytes.remove(&(0x8000 + 127));
        assert!(copy_city_table_observation(&mut partial, base, object).is_err());
        let mut stale = memory.clone();
        stale.put(object + 0x13c, &u32::MAX.to_le_bytes());
        assert!(copy_city_table_observation(&mut stale, base, object).is_err());
        struct ChangedGraph(Fixture, u32);
        impl Memory for ChangedGraph {
            fn copy(&mut self, at: u64, data: &mut [u8]) -> Result<()> {
                if at == 0x8000 + 120 {
                    self.1 += 1;
                    if self.1 == 2 {
                        self.0.u64(at, 0x9010);
                    }
                }
                self.0.copy(at, data)
            }
        }
        let mut changed = ChangedGraph(memory, 0);
        assert!(copy_city_table_observation(&mut changed, base, object).is_err());
    }

    #[test]
    fn city_table_observation_rejects_inactive_owner_without_following_its_player() {
        let mut memory = Fixture {
            bytes: BTreeMap::new(),
            changed: false,
            change_amount: false,
            count: 0,
        };
        let base = 0x180000000;
        let object = 0x6000;

        memory.u64(object, base + 0xab4f58);
        memory.put(object + 0x138, &[2, 0, 2, 0]);
        let full_id = 0x10005u32;
        memory.put(object + 0x13c, &full_id.to_le_bytes());
        memory.u64(object + 0x140, 0);

        memory.u64(base + 0xb8aa68, 0x1000);
        memory.u64(0x1000 + 0x240, 0x2000);
        memory.u64(0x2000 + 0x2b48, 0x3000);
        memory.put(0x3000 + 2 * 4, &(-1i32).to_le_bytes()); // inactive!

        assert!(copy_city_table_observation(&mut memory, base, object).is_err());
    }
}
