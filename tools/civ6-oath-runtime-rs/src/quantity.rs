//! Arithmetic model for a completed military oath's contribution.
//!
//! The city array and saves retain the original Q8 representation. A future
//! native provider must establish the contribution's identity, target, lifetime
//! and completed Apply/Remove before supplying `oath_q8`, `oath_percent`, or
//! a historical removal residual. No residual is inferred from a city value.
//! This module does not establish those facts, install a hook, or accept a game.

use crate::call_identity::{Error, Result};

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
    military_with_residual(input, 0)
}

/// Like `military`, but also exclude a separately confirmed, signed residue
/// accumulated by completed named oath removals. The residue is not an active
/// percentage: it can persist while OFF and through later reapplications.
/// Callers must establish and restore its provenance separately, including
/// after a new-process load; a negative city coefficient alone is insufficient.
pub fn military_with_residual(
    input: MilitaryInput,
    confirmed_residual_q8: i32,
) -> Result<MilitaryQuantity> {
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
    let baseline_city = i32_quantity(
        i128::from(input.city_q8) - i128::from(input.oath_q8) - i128::from(confirmed_residual_q8),
    )?;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuantityError {
    Ledger(crate::contribution_ledger::LedgerError),
    Arithmetic(Error),
}
impl std::fmt::Display for QuantityError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{self:?}")
    }
}
impl std::error::Error for QuantityError {}

/// Quantity binding for a runtime-admitted, current city incarnation. The
/// ledger denies pending/unknown/stale identities before arithmetic occurs.
/// It does not establish the native caller's host holding interval itself.
pub fn from_ledger(
    ledger: &crate::contribution_ledger::ContributionLedger,
    city: &crate::contribution_ledger::CityIdentity,
    unit_index: u32,
    native: crate::contribution_ledger::NativeQuantityFeed,
) -> std::result::Result<MilitaryQuantity, QuantityError> {
    let input = ledger
        .quantity_input(city, unit_index, native)
        .map_err(QuantityError::Ledger)?;
    military_with_residual(
        MilitaryInput {
            owner_q8: input.native.owner_q8,
            city_q8: input.native.city_q8,
            other_rate_q8: input.native.other_rate_q8,
            base_q8: input.native.base_q8,
            oath_q8: input.named.active_raw_q8,
            oath_percent: input.named.declared_pct,
        },
        input.named.removal_residue_q8,
    )
    .map_err(QuantityError::Arithmetic)
}

#[cfg(test)]
mod binding_tests {
    use super::*;
    use crate::contribution_ledger::*;
    fn city(epoch: u64, address: u64) -> CityIdentity {
        CityIdentity {
            stable: StableCityIdentity {
                world_id: "save-world-1".into(),
                birth_generation: 3,
                owner: 0,
                full_id: 65536,
            },
            load_epoch: epoch,
            receiver_address: address,
        }
    }
    fn effect(binding: u64) -> EffectIdentity {
        EffectIdentity {
            stable: StableEffectIdentity {
                definition_name: "JAC_OATH_ODA_TENKA_FUBU_E1".into(),
                stable_instance_id: 464,
            },
            instance_binding: binding,
        }
    }
    fn native(city_q8: i32) -> NativeQuantityFeed {
        NativeQuantityFeed {
            owner_q8: 256,
            city_q8,
            other_rate_q8: 256,
            base_q8: 4096 * 256,
        }
    }
    #[test]
    fn codec_rebind_and_arithmetic_keep_off_residue_and_reapplied_rate() {
        let first_city = city(1, 0x1000);
        let mut ledger = ContributionLedger::new(1, Limits::default()).unwrap();
        ledger
            .start_proven_boundary(first_city.clone(), 132, ProvenStartBoundary::Birth)
            .unwrap();
        let first = ledger
            .begin(&first_city, effect(1), UpdateKind::Apply)
            .unwrap();
        assert!(matches!(
            from_ledger(&ledger, &first_city, 7, native(0)),
            Err(QuantityError::Ledger(LedgerError::Pending))
        ));
        ledger
            .finish(
                first,
                &first_city,
                AdmittedCompletion::Applied {
                    declared_pct: 15,
                    units: vec![UnitDelta {
                        unit_index: 7,
                        raw_q8_delta: 38,
                    }],
                },
            )
            .unwrap();
        let on = from_ledger(&ledger, &first_city, 7, native(38)).unwrap();
        assert_eq!(on.corrected_q8, 1_205_862);
        let remove = ledger
            .begin(&first_city, effect(1), UpdateKind::Remove)
            .unwrap();
        ledger
            .finish(
                remove,
                &first_city,
                AdmittedCompletion::Removed {
                    units: vec![UnitDelta {
                        unit_index: 7,
                        raw_q8_delta: -39,
                    }],
                },
            )
            .unwrap();
        let off = from_ledger(&ledger, &first_city, 7, native(-1)).unwrap();
        assert_eq!(off.corrected_q8, 1_048_576);
        let value =
            crate::contribution_save::encode(&ledger.snapshot(), Limits::default()).unwrap();
        let saved =
            crate::contribution_save::decode(&value, Limits::default(), "save-world-1").unwrap();
        let new_city = city(2, 0x9000);
        let mut restored = ContributionLedger::new(2, Limits::default()).unwrap();
        restored
            .restore_checkpoint(
                &saved,
                &[RestoreBinding {
                    city: new_city.clone(),
                    effects: vec![effect(9)],
                }],
            )
            .unwrap();
        assert!(from_ledger(&restored, &first_city, 7, native(-1)).is_err());
        assert_eq!(
            from_ledger(&restored, &new_city, 7, native(-1)).unwrap(),
            off
        );
        let reapply = restored
            .begin(&new_city, effect(9), UpdateKind::Apply)
            .unwrap();
        restored
            .finish(
                reapply,
                &new_city,
                AdmittedCompletion::Applied {
                    declared_pct: 15,
                    units: vec![UnitDelta {
                        unit_index: 7,
                        raw_q8_delta: 38,
                    }],
                },
            )
            .unwrap();
        let again = from_ledger(&restored, &new_city, 7, native(37)).unwrap();
        assert_eq!(again.counterfactual_q8, 1_048_576);
        assert_eq!(again.corrected_q8, on.corrected_q8);
        assert_eq!(
            from_ledger(&restored, &new_city, 8, native(-64))
                .unwrap()
                .corrected_q8,
            786_432
        );
    }
    #[test]
    fn pending_checkpoint_cannot_become_quantity_authority_after_load() {
        let old = city(1, 0x1000);
        let mut ledger = ContributionLedger::new(1, Limits::default()).unwrap();
        ledger
            .start_proven_boundary(old.clone(), 132, ProvenStartBoundary::Birth)
            .unwrap();
        let _pending = ledger.begin(&old, effect(1), UpdateKind::Apply).unwrap();
        let saved = crate::contribution_save::decode(
            &crate::contribution_save::encode(&ledger.snapshot(), Limits::default()).unwrap(),
            Limits::default(),
            "save-world-1",
        )
        .unwrap();
        let new = city(2, 0x9000);
        let mut restored = ContributionLedger::new(2, Limits::default()).unwrap();
        restored
            .restore_checkpoint(
                &saved,
                &[RestoreBinding {
                    city: new.clone(),
                    effects: vec![],
                }],
            )
            .unwrap();
        assert!(matches!(
            from_ledger(&restored, &new, 7, native(0)),
            Err(QuantityError::Ledger(LedgerError::Unknown(
                UnknownReason::Incomplete
            )))
        ));
    }
}
