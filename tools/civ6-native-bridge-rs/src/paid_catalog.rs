//! Versioned, bool-only transport of synchronized GameInfo definitions.
//! Numeric property encodings are deliberately not assumed. All 22 records
//! are committed together; these definitions never establish payment.
use crate::governor_payment::PaymentDefinition;

pub const COUNT: usize = 22;
pub const WIDTH: usize = 46;
pub const BITS: usize = COUNT * WIDTH;
pub const COMMIT_KEY: u32 = 904_999;
pub const SCHEMA: &str = "JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA";
pub const EXPERIMENT: &str = "JAC_TEST853_PAID_ASSIGNMENT_V1";

pub const fn bit_key(bit: usize) -> u32 {
    if bit < 744 {
        905_256 + bit as u32
    } else {
        906_256 + (bit - 744) as u32
    }
}

pub fn bit_index(key: u32) -> Option<usize> {
    if (905_256..906_000).contains(&key) {
        Some((key - 905_256) as usize)
    } else if (906_256..906_524).contains(&key) {
        Some((key - 906_256) as usize + 744)
    } else {
        None
    }
}

pub fn decode(words: [u64; COUNT]) -> Option<[PaymentDefinition; COUNT]> {
    let mut result = [PaymentDefinition {
        governor_index: 0,
        cost: 1,
        base: 0,
        titles: [None; 2],
    }; COUNT];
    let mut governors = [false; 256];
    let mut promotions = [false; 4096];
    for (slot, word) in words.into_iter().enumerate() {
        if word >> WIDTH != 0 {
            return None;
        }
        let index = (word & 255) as usize;
        let cost = ((word >> 8) & 3) as u8;
        let base = ((word >> 10) & 4095) as u32;
        let one = ((word >> 22) & 4095) as u32;
        let two = ((word >> 34) & 4095) as u32;
        if cost == 0 || governors[index] || (cost == 1 && one != 0) || (cost < 3 && two != 0) {
            return None;
        }
        governors[index] = true;
        let definition = PaymentDefinition {
            governor_index: index as u32,
            cost,
            base,
            titles: [(cost >= 2).then_some(one), (cost == 3).then_some(two)],
        };
        for promotion in std::iter::once(base).chain(definition.titles.into_iter().flatten()) {
            if promotions[promotion as usize] {
                return None;
            }
            promotions[promotion as usize] = true;
        }
        result[slot] = definition;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn namespace_is_disjoint_and_catalog_is_complete_and_unique() {
        let keys: std::collections::BTreeSet<_> = (0..BITS).map(bit_key).collect();
        assert_eq!(keys.len(), BITS);
        for n in 0..BITS {
            assert_eq!(bit_index(bit_key(n)), Some(n));
        }
        for base in [
            900_000, 901_000, 902_000, 903_000, 904_000, 905_000, 906_000, 907_000,
        ] {
            for index in 0..256 {
                assert!(!keys.contains(&(base + index)));
            }
        }
        assert!(!keys.contains(&COMMIT_KEY));
        let words = std::array::from_fn(|i| i as u64 | (1 << 8) | ((100 + i as u64) << 10));
        assert_eq!(decode(words).unwrap()[21].base, 121);
        let mut duplicate = words;
        duplicate[21] = words[0];
        assert!(decode(duplicate).is_none());
        assert!(decode([0; COUNT]).is_none());
    }
}
