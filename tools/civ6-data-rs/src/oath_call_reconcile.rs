//! Reconcile copied call metadata with recorded official inventory, never a
//! completed City contribution, ABA/lifetime guarantee or accepted quantity.
use super::*;
use std::collections::BTreeMap;

fn snapshot_step(name: &str) -> Result<u64> {
    name.strip_prefix("oath-observation-")
        .and_then(|s| s.strip_suffix("-oath_trace_snapshot.json"))
        .context("invalid snapshot filename")?
        .parse::<u64>()
        .context("snapshot step is not an integer")
}

fn copied_family(trace: &Value, row: &Value) -> Option<&'static str> {
    let hex = |v: &Value| {
        v.as_str()?
            .strip_prefix("0x")
            .and_then(|s| u64::from_str_radix(s, 16).ok())
    };
    let rva = hex(&row["effect_vtable"])?.checked_sub(hex(&trace["image_base"])?)?;
    match rva {
        0xa76a30 => Some("military"),
        0xa6cdb8 => Some("naval"),
        0xa90328 => Some("build"),
        0xa9e0f0 => Some("growth"),
        _ => None,
    }
}

fn reconcile(
    trace: &Value,
    inventory: &Value,
    expected: &BTreeMap<String, (String, i64)>,
) -> Result<Value> {
    let version = trace["trace_version"]
        .as_u64()
        .context("trace version missing")?;
    let callee = trace["capture"] == "callee";
    ensure!(
        if callee {
            version == 1
        } else {
            matches!(version, 4 | 5)
        },
        "unsupported trace version"
    );
    ensure!(
        trace["accepted"] == false && trace["quantity_accepted"] == false,
        "observation must not claim acceptance"
    );
    let rows = trace["rows"].as_array().context("trace rows missing")?;
    ensure!(
        rows.len() <= 1024 && trace["row_count"].as_u64() == Some(rows.len() as u64),
        "trace row count mismatch"
    );
    let frames = inventory["observations"]
        .as_array()
        .context("official inventory missing")?;
    let mut definitions: BTreeMap<u64, String> = BTreeMap::new();
    let mut city_sets = Vec::new();
    for frame in frames {
        let mut frame_ids = BTreeSet::new();
        for bucket in ["instances", "other_definitions"] {
            for instance in frame[bucket]
                .as_array()
                .context("inventory definitions missing")?
            {
                let id = instance["id"]
                    .as_u64()
                    .context("invalid official instance id")?;
                ensure!(frame_ids.insert(id), "duplicate official instance id");
                let name = instance["definition"]
                    .as_str()
                    .context("official definition missing")?;
                if let Some(previous) = definitions.insert(id, name.to_owned()) {
                    ensure!(
                        previous == name,
                        "instance id changed definition between recorded frames"
                    );
                }
            }
        }
        let mut cities = BTreeSet::new();
        for city in frame["human_cities"]
            .as_array()
            .context("recorded human cities missing")?
        {
            let id = city["id"].as_u64().context("invalid public city id")?;
            ensure!(cities.insert(id), "duplicate public city id");
        }
        city_sets.push(cities);
    }
    let mut sequences = BTreeSet::new();
    let mut output = Vec::new();
    for row in rows {
        let sequence = row["sequence"].as_u64().context("call sequence missing")?;
        ensure!(
            sequence > 0 && sequences.insert(sequence),
            "duplicate or invalid call sequence"
        );
        let metadata = &row["call_entry_metadata"];
        let id = if callee {
            metadata["instance_id"].as_u64()
        } else {
            Some(
                row["instance_id"]
                    .as_u64()
                    .context("call instance id missing")?,
            )
        };
        let stable = matches!(
            row["state"].as_str(),
            Some("completed" | "incomplete" | "unwound")
        );
        let definition = if callee && !stable {
            None
        } else {
            id.and_then(|id| definitions.get(&id))
        };
        let spec = definition.and_then(|name| expected.get(name));
        let family = if callee {
            if stable {
                match row["slot"].as_u64() {
                    Some(0 | 1) => Some("military"),
                    Some(2 | 3) => Some("naval"),
                    Some(4 | 5) => Some("build"),
                    Some(6 | 7) => Some("growth"),
                    _ => None,
                }
            } else {
                None
            }
        } else if row["copy_flags"]
            .as_u64()
            .is_some_and(|flags| flags & 1 != 0)
            && !matches!(
                row["state"].as_str(),
                Some("unknown_unstable" | "unknown_unpublished") | None
            )
        {
            copied_family(trace, row)
        } else {
            None
        };
        let copied = stable
            && if callee {
                metadata.is_object() && id.is_some()
            } else {
                version == 5 && metadata["identity_copied"] == true
            };
        let declared = if copied {
            metadata["declared_percent"].as_i64()
        } else {
            None
        };
        let city = if copied && callee {
            Some(json!({"owner":metadata["owner"],"full_id":metadata["full_id"]}))
        } else if copied {
            Some(metadata["indicated_city"].clone())
        } else {
            None
        };
        let public_match = city.as_ref().and_then(|city| {
            let city = city.as_object()?;
            let owner = city.get("owner")?.as_u64()?;
            let full_id = city.get("full_id")?.as_u64()?;
            Some(
                owner == 0
                    && !city_sets.is_empty()
                    && city_sets.iter().all(|ids| ids.contains(&full_id)),
            )
        });
        output.push(json!({
            "sequence":sequence,"call":row["call"],"state":row["state"],"instance_id":id,
            "official_definition":definition,"catalogue_kind":spec.map(|s| &s.0),
            "catalogue_percent":spec.map(|s| s.1),"declared_percent":declared,
            "copied_effect_family":family,"copied_family_matches_catalogue":family.zip(spec).map(|(f,s)| f == if s.0 == "walls" { "build" } else { &s.0 }),
            "declared_percent_matches_catalogue":declared.zip(spec).map(|(amount,s)| amount==s.1),
            "indicated_city":city,
            "city_identity_source":if callee {"callee_rdx_backpointer_copy"} else {"adapter_subject_copy"},
            "actual_receiver_argument":if callee && stable {row["actual_receiver_argument"].clone()} else {Value::Null},
            "copied_definition_address":if copied {metadata["definition"].clone()} else {Value::Null},
            "indicated_city_present_in_all_recorded_human_frames":public_match,
            "original_al":row["original_al"],
            "original_rax":row["original_rax"],
            "limitations":["official instance id may be reused between frames", "copied City identity does not establish lifetime or ABA exclusion", "completed return does not measure a contribution", "callee family does not establish unit or building target filters"]
        }));
    }
    Ok(
        json!({"scope":"recorded_call_inventory_reconciliation", "trace_version":version,"capture":if callee {"callee"} else {"adapter"},
        "trace_complete":trace["complete"],"inventory_available":frames.iter().map(|f| f["inventory_available"].clone()).collect::<Vec<_>>(),
        "rows":output,"quantity_accepted":false,"product_accepted":false,
        "limitations":["no live process was read", "no quantity or City lifetime is established", "v4 has no copied identity metadata"]}),
    )
}

pub(super) fn read(repo: &Repository, directory: &Path) -> Result<Value> {
    let inventory = super::read_modifier_inventory(directory)?;
    let mut expected = BTreeMap::new();
    for case in super::cases(repo)? {
        for (index, effect) in case.effects.into_iter().enumerate() {
            expected.insert(
                format!("{}_E{}", case.oath, index + 1),
                (effect.kind, effect.amount),
            );
        }
    }
    let mut snapshots = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("oath-observation-") && name.ends_with("-oath_trace_snapshot.json") {
            let step = snapshot_step(&name)?;
            snapshots.push((step, name, entry.path()));
        }
    }
    snapshots.sort_by_key(|s| s.0);
    ensure!(
        snapshots.windows(2).all(|s| s[0].0 != s[1].0),
        "duplicate native snapshot step"
    );
    ensure!(!snapshots.is_empty(), "no recorded native snapshot");
    let mut result = Vec::new();
    for (_, name, path) in snapshots {
        let original: Value = serde_json::from_slice(&std::fs::read(path)?)?;
        ensure!(
            original["diagnostic_only"] == true
                && original["quantity_accepted"] == false
                && original["product_accepted"] == false,
            "native artifact claims acceptance"
        );
        result.push(json!({"artifact":name,"reconciliation":reconcile(&original["native_observation"], &inventory,&expected)?}));
    }
    Ok(json!({"snapshots":result,"quantity_accepted":false,"product_accepted":false}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_steps_follow_numeric_order_and_reject_malformed_names() -> Result<()> {
        let mut names = [
            "oath-observation-10-oath_trace_snapshot.json",
            "oath-observation-9-oath_trace_snapshot.json",
        ];
        names.sort_by_key(|name| snapshot_step(name).unwrap());
        assert_eq!(snapshot_step(names[0])?, 9);
        assert!(snapshot_step("oath-observation-x-oath_trace_snapshot.json").is_err());
        assert!(snapshot_step("oath-observation-9-oath_trace_stop.json").is_err());
        Ok(())
    }
    fn fixture() -> (Value, Value, BTreeMap<String, (String, i64)>) {
        let trace = json!({"trace_version":5,"accepted":false,"quantity_accepted":false,"row_count":1,"complete":true,"image_base":"0x180000000","rows":[{
            "sequence":1,"call":"apply","state":"completed","instance_id":464,"original_al":1,"copy_flags":7,"effect_vtable":"0x180a76a30",
            "call_entry_metadata":{"identity_copied":true,"declared_percent":15,"indicated_city":{"owner":0,"full_id":65536}}
        }]});
        let inventory = json!({"observations":[{"inventory_available":false,"instances":[{"id":464,"definition":"JAC_OATH_ODA_TENKA_FUBU_E1"}],"other_definitions":[],"human_cities":[{"id":65536}]}]});
        (
            trace,
            inventory,
            [("JAC_OATH_ODA_TENKA_FUBU_E1".into(), ("military".into(), 15))].into(),
        )
    }
    #[test]
    fn call_copies_join_official_names_without_accepting_quantity_or_active_flag() -> Result<()> {
        let (trace, inventory, expected) = fixture();
        let value = reconcile(&trace, &inventory, &expected)?;
        assert_eq!(value["rows"][0]["declared_percent_matches_catalogue"], true);
        assert_eq!(value["rows"][0]["copied_family_matches_catalogue"], true);
        assert_eq!(
            value["rows"][0]["indicated_city_present_in_all_recorded_human_frames"],
            true
        );
        assert_eq!(value["inventory_available"][0], false);
        assert_eq!(value["quantity_accepted"], false);
        Ok(())
    }

    #[test]
    fn callee_rows_join_the_same_official_inventory_and_preserve_unknown_metadata() -> Result<()> {
        let (mut trace, inventory, expected) = fixture();
        trace["capture"] = json!("callee");
        trace["trace_version"] = json!(1);
        trace["rows"][0] = json!({
            "sequence":1,"slot":0,"call":"apply","state":"completed",
            "actual_receiver_argument":"0x9000","original_rax":"0x1234",
            "call_entry_metadata":{"instance_id":464,"declared_percent":15,"owner":0,"full_id":65536,"definition":"0x6000"}
        });
        let value = reconcile(&trace, &inventory, &expected)?;
        assert_eq!(value["capture"], "callee");
        let row = &value["rows"][0];
        assert_eq!(row["official_definition"], "JAC_OATH_ODA_TENKA_FUBU_E1");
        assert_eq!(row["copied_family_matches_catalogue"], true);
        assert_eq!(row["declared_percent_matches_catalogue"], true);
        assert_eq!(
            row["indicated_city_present_in_all_recorded_human_frames"],
            true
        );
        assert_eq!(row["actual_receiver_argument"], "0x9000");
        assert_eq!(row["original_rax"], "0x1234");
        assert!(row["original_al"].is_null());
        trace["rows"][0]["call_entry_metadata"] = Value::Null;
        let unknown = reconcile(&trace, &inventory, &expected)?;
        for field in [
            "instance_id",
            "official_definition",
            "declared_percent",
            "indicated_city",
            "copied_definition_address",
        ] {
            assert!(unknown["rows"][0][field].is_null(), "{field}");
        }
        assert_eq!(unknown["quantity_accepted"], false);
        assert_eq!(unknown["product_accepted"], false);
        Ok(())
    }

    #[test]
    fn callee_capture_cannot_be_confused_with_adapter_version_or_unstable_identity() -> Result<()> {
        let (mut trace, inventory, expected) = fixture();
        trace["capture"] = json!("callee");
        assert!(reconcile(&trace, &inventory, &expected).is_err());
        trace["trace_version"] = json!(1);
        trace["rows"][0]["call_entry_metadata"] =
            json!({"instance_id":464,"declared_percent":15,"owner":0,"full_id":65536});
        trace["rows"][0]["slot"] = json!(0);
        trace["rows"][0]["state"] = json!("unknown_unstable");
        let value = reconcile(&trace, &inventory, &expected)?;
        for field in [
            "official_definition",
            "declared_percent",
            "copied_effect_family",
            "indicated_city",
            "actual_receiver_argument",
        ] {
            assert!(value["rows"][0][field].is_null(), "{field}");
        }
        trace["capture"] = json!("adapter");
        assert!(reconcile(&trace, &inventory, &expected).is_err());
        Ok(())
    }
    #[test]
    fn missing_v4_metadata_unstable_copy_and_unknown_definitions_remain_unknown() -> Result<()> {
        let (mut trace, inventory, expected) = fixture();
        trace["trace_version"] = 4.into();
        let row = reconcile(&trace, &inventory, &expected)?["rows"][0].clone();
        assert!(row["declared_percent"].is_null());
        assert!(row["indicated_city"].is_null());
        trace["trace_version"] = 5.into();
        trace["rows"][0]["call_entry_metadata"]["identity_copied"] = false.into();
        assert!(reconcile(&trace, &inventory, &expected)?["rows"][0]["declared_percent"].is_null());
        trace["rows"][0]["instance_id"] = 999.into();
        assert!(
            reconcile(&trace, &inventory, &expected)?["rows"][0]["official_definition"].is_null()
        );
        Ok(())
    }
    #[test]
    fn exact_generation_other_owner_and_declared_rate_mismatches_are_observable() -> Result<()> {
        let (mut trace, inventory, expected) = fixture();
        for city in [
            json!({"owner":0,"full_id":131072}),
            json!({"owner":1,"full_id":65536}),
        ] {
            trace["rows"][0]["call_entry_metadata"]["indicated_city"] = city;
            trace["rows"][0]["call_entry_metadata"]["declared_percent"] = 14.into();
            let value = reconcile(&trace, &inventory, &expected)?;
            assert_eq!(
                value["rows"][0]["declared_percent_matches_catalogue"],
                false
            );
            assert_eq!(
                value["rows"][0]["indicated_city_present_in_all_recorded_human_frames"],
                false
            );
        }
        Ok(())
    }
    #[test]
    fn family_underflow_unreadable_and_unknown_addresses_do_not_become_military() -> Result<()> {
        let (mut trace, inventory, expected) = fixture();
        for address in ["0x1", "0x180a90328", "bad", "0x180a76a31"] {
            trace["rows"][0]["effect_vtable"] = address.into();
            let value = reconcile(&trace, &inventory, &expected)?;
            if address == "0x180a90328" {
                assert_eq!(value["rows"][0]["copied_effect_family"], "build");
                assert_eq!(value["rows"][0]["copied_family_matches_catalogue"], false);
            } else {
                assert!(value["rows"][0]["copied_family_matches_catalogue"].is_null());
            }
        }
        trace["rows"][0]["effect_vtable"] = "0x180a76a30".into();
        trace["rows"][0]["state"] = "unknown_unstable".into();
        assert!(
            reconcile(&trace, &inventory, &expected)?["rows"][0]["copied_effect_family"].is_null()
        );
        Ok(())
    }
    #[test]
    fn walls_catalogue_matches_the_build_family_without_proving_its_filter() -> Result<()> {
        let (mut trace, inventory, mut expected) = fixture();
        trace["rows"][0]["effect_vtable"] = "0x180a90328".into();
        expected.get_mut("JAC_OATH_ODA_TENKA_FUBU_E1").unwrap().0 = "walls".into();
        let value = reconcile(&trace, &inventory, &expected)?;
        assert_eq!(value["rows"][0]["copied_effect_family"], "build");
        assert_eq!(value["rows"][0]["catalogue_kind"], "walls");
        assert_eq!(value["rows"][0]["copied_family_matches_catalogue"], true);
        assert_eq!(value["quantity_accepted"], false);
        assert!(value["rows"][0]["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().is_some_and(|s| s.contains("target filters"))));
        Ok(())
    }
    #[test]
    fn reused_official_id_duplicate_sequence_and_acceptance_claims_are_rejected() {
        let (mut trace, mut inventory, expected) = fixture();
        let other_frame = inventory["observations"][0].clone();
        inventory["observations"]
            .as_array_mut()
            .unwrap()
            .push(other_frame);
        inventory["observations"][1]["instances"][0]["definition"] = "UNRELATED".into();
        assert!(reconcile(&trace, &inventory, &expected).is_err());
        inventory["observations"].as_array_mut().unwrap().pop();
        let duplicate = trace["rows"][0].clone();
        trace["rows"].as_array_mut().unwrap().push(duplicate);
        trace["row_count"] = 2.into();
        assert!(reconcile(&trace, &inventory, &expected).is_err());
        trace["accepted"] = true.into();
        assert!(reconcile(&trace, &inventory, &expected).is_err());
    }
}
