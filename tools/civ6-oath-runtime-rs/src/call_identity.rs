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
    /// Point-in-time interface table only; no getter is called or name inferred.
    pub definition_vtable_rva: Option<u32>,
    pub definition_name: Option<DefinitionName>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DefinitionName {
    pub bytes: [u8; 64],
    pub len: u32,
}

/// Copy the pinned Id getter's readonly string reference without calling it.
/// The observed interface table and leaf return define this exact layout.
/// This copies a label at call entry, not any lifetime or quantity authority.
fn definition_name(
    memory: &mut (impl Memory + ?Sized),
    base: u64,
    definition: u64,
) -> Result<DefinitionName> {
    if u64::from_le_bytes(read(memory, definition)?) != address(base, 0xaa3f20)?
        || u64::from_le_bytes(read(memory, address(base, 0xaa3f28)?)?) != address(base, 0x954cc0)?
        || read::<5>(memory, address(base, 0x954cc0)?)? != [0x48, 0x8d, 0x41, 0x18, 0xc3]
    {
        return Err(Error::input("unsupported definition Id interface"));
    }
    let string = address(definition, 0x18)?;
    let length = u64::from_le_bytes(read(memory, address(string, 0x10)?)?);
    let capacity = u64::from_le_bytes(read(memory, address(string, 0x18)?)?);
    if length == 0 || length > 64 || capacity < length || capacity < 15 {
        return Err(Error::input("unsupported definition Id string shape"));
    }
    let data = if capacity == 15 {
        string
    } else {
        let data = u64::from_le_bytes(read(memory, string)?);
        if data == 0 {
            return Err(Error::input("null definition Id string"));
        }
        data
    };
    let mut bytes = [0u8; 65];
    memory.copy(data, &mut bytes[..length as usize + 1])?;
    if bytes[length as usize] != 0
        || !bytes[..length as usize]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        return Err(Error::input("invalid definition Id bytes"));
    }
    let mut name = DefinitionName {
        bytes: [0; 64],
        len: length as u32,
    };
    name.bytes[..length as usize].copy_from_slice(&bytes[..length as usize]);
    Ok(name)
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
            (1, 0x974287 | 0x974364) => rdi,
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
            definition_vtable_rva: read::<8>(memory, definition)
                .ok()
                .map(u64::from_le_bytes)
                .and_then(|table| table.checked_sub(base))
                .filter(|rva| (0x9b7000..0xb5f124).contains(rva) && rva & 7 == 0)
                .and_then(|rva| u32::try_from(rva).ok()),
            definition_name: definition_name(memory, base, definition).ok(),
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
        (Call::Remove, 0x974287 | 0x974364) => frame.rdi,
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
        assert_eq!(identity.definition_vtable_rva, None);
        memory.u64(0x6000, base + 0xab1230);
        assert_eq!(
            read(&mut memory, 0, city).unwrap().definition_vtable_rva,
            Some(0xab1230)
        );
        memory.u64(0x6000, base + 0x1000);
        assert_eq!(
            read(&mut memory, 0, city).unwrap().definition_vtable_rva,
            None
        );
        memory.u64(0x6000, base + 0xab1231);
        assert_eq!(
            read(&mut memory, 0, city).unwrap().definition_vtable_rva,
            None
        );
        memory.bytes.remove(&0x6007);
        assert_eq!(
            read(&mut memory, 0, city).unwrap().definition_vtable_rva,
            None
        );
        assert!(read(&mut memory, 2, city).is_err());
        assert!(read(&mut memory, 0, city + 8).is_err());
        assert!(read(&mut memory, 1, city).is_err());
        memory.u64(city + 0x1968, 0);
        assert!(read(&mut memory, 0, city).is_err());
    }

    fn named_definition(base: u64, name: &[u8], heap: bool) -> Fixture {
        let (mut memory, _) = fixture(base);
        memory.u64(0x6000, base + 0xaa3f20);
        memory.u64(base + 0xaa3f28, base + 0x954cc0);
        memory.put(base + 0x954cc0, &[0x48, 0x8d, 0x41, 0x18, 0xc3]);
        memory.u64(0x6028, name.len() as u64);
        memory.u64(0x6030, if heap { 127 } else { 15 });
        let data = if heap {
            memory.u64(0x6018, 0x9001);
            0x9001
        } else {
            0x6018
        };
        memory.put(data, name);
        memory.put(data + name.len() as u64, &[0]);
        memory
    }

    #[test]
    fn definition_id_copy_supports_inline_heap_and_exact_bounded_length_without_getter_calls() {
        let base = 0x180000000;
        for (label, heap) in [
            (b"SIMPLE_ID".as_slice(), false),
            (b"JAC_OATH_ODA_TENKA_FUBU_E1".as_slice(), true),
            (&[b'A'; 64], true),
        ] {
            let mut memory = named_definition(base, label, heap);
            let name = definition_name(&mut memory, base, 0x6000).unwrap();
            assert_eq!(name.len as usize, label.len());
            assert_eq!(&name.bytes[..label.len()], label);
        }
        for case in 0..8 {
            let mut memory = named_definition(base, b"JAC_OATH_ODA_TENKA_FUBU_E1", true);
            match case {
                0 => memory.u64(0x6000, base + 0xaa3f28),
                1 => memory.u64(base + 0xaa3f28, base + 0x954cd0),
                2 => memory.put(base + 0x954cc0, &[0x90]),
                3 => memory.u64(0x6028, 65),
                4 => memory.u64(0x6030, 15),
                5 => memory.u64(0x6018, 0),
                6 => memory.put(0x9001, b"!"),
                7 => {
                    memory
                        .bytes
                        .remove(&(0x9001 + b"JAC_OATH_ODA_TENKA_FUBU_E1".len() as u64));
                }
                _ => unreachable!(),
            };
            assert!(
                definition_name(&mut memory, base, 0x6000).is_err(),
                "case {case}"
            );
        }
    }

    #[test]
    fn changing_definition_name_in_the_same_callee_copy_is_unknown() {
        let base = 0x180000000;
        let mut memory = named_definition(base, b"JAC_OATH_ODA_TENKA_FUBU_E1", true);
        memory.u64(0x10000 + 0x1968, 0x8000);
        memory.u64(0x8000, base + 0xab4f58);
        memory.put(0x8138, &[0, 0, 2, 0]);
        memory.put(0x813c, &65536u32.to_le_bytes());
        memory.u64(0x8140, 0);
        struct ChangedName(Fixture, u32);
        impl Memory for ChangedName {
            fn copy(&mut self, at: u64, bytes: &mut [u8]) -> Result<()> {
                if at == 0x9001 {
                    self.1 += 1;
                    if self.1 == 2 {
                        self.0.put(at, b"K");
                    }
                }
                self.0.copy(at, bytes)
            }
        }
        let frame = RecipientFrame {
            base,
            slot: 0,
            caller: base + 0x944f27,
            rbp: 0x2000,
            rdi: 0,
            effect: 0x4000,
            actual_receiver: 0x10000,
        };
        assert_eq!(
            copy_recipient_identity(&mut memory, frame)
                .unwrap()
                .definition_name
                .unwrap()
                .bytes[0],
            b'J'
        );
        assert!(copy_recipient_identity(&mut ChangedName(memory, 0), frame).is_err());
    }

    #[test]
    fn all_known_caller_frames_use_the_correct_nonvolatile_receiver() {
        for (call, caller) in [
            (Call::Apply, 0x944e55),
            (Call::Apply, 0x944f27),
            (Call::Remove, 0x974287),
            (Call::Remove, 0x974364),
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
                memory.u64(0x10000 + 0x1968, 0x8000);
                memory.u64(0x8000, base + 0xab4f58);
                memory.put(0x8138, &[0, 0, 2, 0]);
                memory.put(0x813c, &65536u32.to_le_bytes());
                memory.u64(0x8140, 0);
                let recipient = copy_recipient_identity(
                    &mut memory,
                    RecipientFrame {
                        base,
                        slot: if call == Call::Apply { 0 } else { 1 },
                        caller: frame.caller_return,
                        rbp: frame.rbp,
                        rdi: frame.rdi,
                        effect: frame.effect,
                        actual_receiver: 0x10000,
                    },
                )
                .unwrap();
                assert_eq!(recipient.instance_id, observed.instance_id);
                assert_eq!(recipient.full_id, 65536);
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
