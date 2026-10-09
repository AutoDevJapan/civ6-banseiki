//! Bounded copies of the military coefficient array used by the original
//! callee. These are observations before/after a call, not an attribution,
//! synchronization contract, retained pointer, or authority to change a city.
use crate::call_identity::{Error, Memory, Result};

pub const MAX_UNITS: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionCopy {
    pub array: u64,
    pub length: u32,
    pub coefficients: [i32; MAX_UNITS],
}

fn read<const N: usize>(memory: &mut impl Memory, at: u64) -> Result<[u8; N]> {
    let mut bytes = [0; N];
    memory.copy(at, &mut bytes)?;
    Ok(bytes)
}

/// 72A920 returns its input+10h on both normal return paths. The original
/// military callee passes City+10C0h, then uses returned+2C0h. The serializer
/// 10686A -> F32E0 copies the array pointer at +0 and uint32 length at +10h.
/// No getter, COW operation, or game function is called by this copy.
pub fn copy(memory: &mut impl Memory, city: u64) -> Result<ProductionCopy> {
    if city == 0 || city & 7 != 0 {
        return Err(Error::input("invalid production receiver"));
    }
    let at = city
        .checked_add(0x1390)
        .ok_or_else(|| Error::input("production array address overflow"))?;
    let descriptor = read::<20>(memory, at)?;
    let array = u64::from_le_bytes(descriptor[..8].try_into().unwrap());
    let length = u32::from_le_bytes(descriptor[16..20].try_into().unwrap());
    if length == 0 || length as usize > MAX_UNITS || array == 0 || array & 3 != 0 {
        return Err(Error::input("unsupported production array shape"));
    }
    array
        .checked_add(u64::from(length) * 4)
        .ok_or_else(|| Error::input("production array end overflow"))?;
    let mut bytes = [0; MAX_UNITS * 4];
    let size = length as usize * 4;
    memory.copy(array, &mut bytes[..size])?;
    let mut coefficients = [0; MAX_UNITS];
    for (value, word) in coefficients.iter_mut().zip(bytes[..size].chunks_exact(4)) {
        *value = i32::from_le_bytes(word.try_into().unwrap());
    }
    if read::<20>(memory, at)? != descriptor {
        return Err(Error::input("production descriptor changed while copying"));
    }
    Ok(ProductionCopy {
        array,
        length,
        coefficients,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    struct Fixture(BTreeMap<u64, u8>, bool);
    impl Fixture {
        fn put(&mut self, at: u64, bytes: &[u8]) {
            self.0
                .extend(bytes.iter().enumerate().map(|(i, b)| (at + i as u64, *b)));
        }
    }
    impl Memory for Fixture {
        fn copy(&mut self, at: u64, bytes: &mut [u8]) -> Result<()> {
            for (i, byte) in bytes.iter_mut().enumerate() {
                *byte = *self.0.get(&(at + i as u64)).ok_or(Error("missing bytes"))?;
            }
            if self.1 && at == 0x9004 {
                self.put(0x2390, &0xa000u64.to_le_bytes());
            }
            Ok(())
        }
    }
    fn fixture(length: u32) -> Fixture {
        let mut memory = Fixture(BTreeMap::new(), false);
        let mut descriptor = [0; 20];
        descriptor[..8].copy_from_slice(&0x9004u64.to_le_bytes());
        descriptor[16..20].copy_from_slice(&length.to_le_bytes());
        memory.put(0x2390, &descriptor);
        for i in 0..length.min(MAX_UNITS as u32) {
            memory.put(0x9004 + u64::from(i) * 4, &((i as i32) - 256).to_le_bytes());
        }
        memory
    }
    #[test]
    fn copies_all_signed_coefficients_and_accepts_four_byte_array_alignment() {
        for count in [1, MAX_UNITS as u32] {
            let value = copy(&mut fixture(count), 0x1000).unwrap();
            assert_eq!(value.length, count);
            assert_eq!(value.array, 0x9004);
            assert_eq!(value.coefficients[0], -256);
            assert_eq!(value.coefficients[count as usize - 1], count as i32 - 257);
        }
    }
    #[test]
    fn malformed_missing_and_changed_arrays_are_unknown() {
        for count in [0, MAX_UNITS as u32 + 1, u32::MAX] {
            assert!(copy(&mut fixture(count), 0x1000).is_err());
        }
        for pointer in [0, 0x9001, u64::MAX - 3] {
            let mut memory = fixture(2);
            memory.put(0x2390, &pointer.to_le_bytes());
            assert!(copy(&mut memory, 0x1000).is_err());
        }
        let mut memory = fixture(2);
        memory.0.remove(&0x900b);
        assert!(copy(&mut memory, 0x1000).is_err());
        let mut memory = fixture(2);
        memory.1 = true;
        assert!(copy(&mut memory, 0x1000).is_err());
        assert!(copy(&mut fixture(2), u64::MAX - 7).is_err());
    }
}
