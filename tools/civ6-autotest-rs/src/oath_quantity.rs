//! Arithmetic model for a completed military oath's contribution.
//!
//! The city array and saves retain the original Q8 representation. A future
//! native provider must establish the contribution's identity, target, lifetime
//! and completed Apply/Remove before supplying `oath_q8` and `oath_percent`.
//! This module does not establish those facts, install a hook, or accept a game.

use crate::{Error, Result};

#[derive(Clone, Copy, Debug)]
pub struct MilitaryInput {
    pub owner_q8: i32,
    pub city_q8: i32,
    pub other_rate_q8: i32,
    pub base_q8: i32,
    pub oath_q8: i32,
    pub oath_percent: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MilitaryQuantity {
    pub original_q8: i32,
    pub counterfactual_q8: i32,
    pub corrected_q8: i32,
}

fn i32_quantity(value: i128) -> Result<i32> {
    i32::try_from(value).map_err(|_| Error::input("military quantity outside native int32 range"))
}

// Pinned DLL 1059dd..105a02: signed 32-bit sum, signed 64-bit product and
// arithmetic shift, back to int32, then another product and arithmetic shift.
// A native overflow is not evidence for a meaningful corrected quantity.
fn original(owner: i32, city: i32, other: i32, base: i32) -> Result<i32> {
    let sum = owner
        .checked_add(city)
        .ok_or_else(|| Error::input("military coefficient overflow"))?;
    let first = i32_quantity((i128::from(sum) * i128::from(other)).div_euclid(256))?;
    i32_quantity((i128::from(first) * i128::from(base)).div_euclid(256))
}

/// Preserve the actual non-oath baseline's two rounding stages, remove the
/// confirmed named contribution from that calculation, then add the declared
/// percentage with one final rounding. Changing the entire calculation to a
/// single division would also change the OFF baseline and its 100% reference.
/// Negative values use floor, like the native arithmetic right shifts.
pub fn military(input: MilitaryInput) -> Result<MilitaryQuantity> {
    if input.oath_percent < 0 || input.oath_q8 < 0 {
        return Err(Error::input("positive oath contribution required"));
    }
    if input.oath_percent == 0 && input.oath_q8 != 0 {
        return Err(Error::input(
            "native oath contribution lacks declared percentage",
        ));
    }
    let original_q8 = original(
        input.owner_q8,
        input.city_q8,
        input.other_rate_q8,
        input.base_q8,
    )?;
    let baseline_city = input
        .city_q8
        .checked_sub(input.oath_q8)
        .ok_or_else(|| Error::input("military counterfactual coefficient overflow"))?;
    let counterfactual_q8 = original(
        input.owner_q8,
        baseline_city,
        input.other_rate_q8,
        input.base_q8,
    )?;
    let delta = (i128::from(input.oath_percent)
        * i128::from(input.other_rate_q8)
        * i128::from(input.base_q8))
    .div_euclid(25_600);
    let corrected_q8 = i32_quantity(i128::from(counterfactual_q8) + delta)?;
    Ok(MilitaryQuantity {
        original_q8,
        counterfactual_q8,
        corrected_q8,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(owner: i32, other: i32, city: i32, raw: i32, percent: i32) -> MilitaryInput {
        MilitaryInput {
            owner_q8: owner,
            city_q8: city,
            other_rate_q8: other,
            base_q8: 4096 * 256,
            oath_q8: raw,
            oath_percent: percent,
        }
    }

    #[test]
    fn fifteen_percent_uses_declared_quantity_not_thirty_eight_ticks() {
        // Actual execution of the pinned 37-byte window returned 1_204_224.
        let value = military(input(256, 256, 38, 38, 15)).unwrap();
        assert_eq!(value.original_q8, 1_204_224);
        assert_eq!(value.counterfactual_q8, 1_048_576);
        assert_eq!(value.corrected_q8, 1_205_862);
    }

    #[test]
    fn other_multiplier_does_not_change_the_native_off_baseline() {
        let off = military(input(307, 257, 0, 0, 0)).unwrap();
        let hundred = military(input(307, 257, 256, 0, 0)).unwrap();
        let on = military(input(307, 257, 38, 38, 15)).unwrap();
        assert_eq!(off.original_q8, 1_261_568);
        assert_eq!(on.counterfactual_q8, off.original_q8);
        assert_eq!(on.corrected_q8, 1_419_468);
        // The measured 100% reference and the exact increment differ only
        // by the native result's final reporting tick.
        assert_eq!(hundred.original_q8 - off.original_q8, 1_052_672);
        assert_eq!(on.corrected_q8 - off.original_q8, 157_900);
        assert_eq!(
            military(input(307, 257, 0, 0, 0)).unwrap().corrected_q8,
            off.original_q8
        );
    }

    #[test]
    fn multiple_named_effects_are_rounded_together_without_touching_other_effects() {
        let value = military(input(256, 256, 64 + 12 + 25, 12 + 25, 5 + 10)).unwrap();
        assert_eq!(value.counterfactual_q8, 1_310_720);
        assert_eq!(value.corrected_q8, 1_468_006);
        assert_eq!(
            military(input(256, 256, 64, 0, 0)).unwrap().corrected_q8,
            1_310_720
        );
    }

    #[test]
    fn negative_inputs_use_native_floor_and_invalid_arithmetic_is_not_zero() {
        let mut value = input(-128, 257, 38, 38, 15);
        value.base_q8 = 257;
        let output = military(value).unwrap();
        assert_eq!(output.original_q8, -92);
        assert_eq!(output.counterfactual_q8, -130);
        assert_eq!(output.corrected_q8, -92);
        value = input(256, -257, 38, 38, 15);
        value.base_q8 = 257;
        let output = military(value).unwrap();
        assert_eq!(output.corrected_q8 - output.counterfactual_q8, -39);
        value = input(i32::MAX, 257, 0, 0, 0);
        value.base_q8 = 0;
        assert!(military(value).is_err());
        value = input(-128, 257, 38, 38, 15);
        value.oath_percent = 0;
        assert!(military(value).is_err());
        value = input(i32::MAX, i32::MAX, 38, 38, 15);
        assert!(military(value).is_err());
        value = input(0, 256, i32::MIN, 38, 15);
        assert!(military(value).is_err());
        value = input(256, 256, 38, 38, i32::MAX);
        assert!(military(value).is_err());
    }
}
