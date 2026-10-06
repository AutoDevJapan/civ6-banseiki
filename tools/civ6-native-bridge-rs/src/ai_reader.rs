//! Real UI read -> native catalog query -> immutable reader registration.
//! Missing personal payment remains null, never zero or permission.
use serde_json::{Value, json};
pub fn candidates(value: &Value) -> Result<&Vec<Value>, String> {
    let rows = value["candidates"]
        .as_array()
        .filter(|r| !r.is_empty() && r.len() <= 22)
        .ok_or("requires 1..22 actual AI candidates")?;
    let mut owners = std::collections::BTreeSet::new();
    for row in rows {
        let n = |key: &str| row[key].as_u64();
        if row["source"] != "civ6-persona-events-rs/v3/normal-Gameplay"
            || row["human"] != false
            || row["alive"] != true
            || row["major"] != true
            || row["ledger_active"] != true
            || !n("owner").is_some_and(|o| o < 64 && owners.insert(o))
            || !n("index").is_some_and(|i| i < 256)
            || !matches!(n("cost"), Some(2 | 3))
            || !n("base").is_some_and(|i| i < 4096)
            || !n("title").is_some_and(|i| i < 4096)
            || !n("spent").is_some_and(|i| i <= u64::from(u32::MAX))
            || !n("turn").is_some()
            || !n("total").is_some_and(|i| i <= u64::from(u32::MAX) && Some(i) >= n("spent"))
            || !(row.get("paid") == Some(&Value::Null)
                || n("paid").is_some_and(|p| p <= n("cost").unwrap_or(0)))
        {
            return Err("AI candidate lacks real identity/catalog/normal-ledger read".into());
        }
    }
    Ok(rows)
}
pub fn request(row: &Value, generation: u32) -> Result<Value, String> {
    candidates(&json!({"candidates":[row]}))?;
    if generation == 0 {
        return Err("native query generation missing".into());
    }
    Ok(
        json!({"action":"start_native_ai_observation","mode":"reader","owner":row["owner"],"index":row["index"],
        "cost":row["cost"],"paid":row["paid"],"spent":row["spent"],"title":row["title"],"generation":generation}),
    )
}
pub fn selected(input: &Value, receipt: &Value, pid: u32, created: u64) -> Result<Value, String> {
    let actual = receipt
        .get("actual_request")
        .ok_or("actual-request absent")?;
    let command = crate::ai_diagnostic::Command::parse(actual, pid, created)?;
    if !command.reader_only || command.stop {
        return Err("not a reader registration".into());
    }
    if !candidates(input)?
        .iter()
        .any(|row| request(row, command.subject.generation).ok().as_ref() == Some(actual))
    {
        return Err("native selection differs from actual UI candidates".into());
    }
    crate::ai_diagnostic::validate_matched_receipt(receipt, command)?;
    Ok(actual.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row() -> Value {
        json!({"source":"civ6-persona-events-rs/v3/normal-Gameplay","human":false,"alive":true,"major":true,
        "ledger_active":true,"owner":2,"index":9,"cost":2,"base":70,"title":71,"spent":3,"total":5,"turn":12,"paid":null})
    }
    #[test]
    fn actual_reader_null_payment_is_preserved_and_never_grants_payment() {
        let request = request(&row(), 19).unwrap();
        assert!(request["paid"].is_null());
        let command = crate::ai_diagnostic::Command::parse(&request, 22, 33).unwrap();
        let once = crate::ai_once::Once::observing_reader(command.subject).unwrap();
        assert_eq!(
            once.claim(command.subject),
            Err(crate::ai_once::Reject::Lifecycle)
        );
        assert_eq!(
            once.request_execution(),
            Err(crate::ai_once::Reject::Lifecycle)
        );
        assert!(crate::ai_once::Once::observing(command.subject).is_err());
    }
    #[test]
    fn reader_rejects_fabricated_missing_or_duplicate_subject_and_zero_generation() {
        assert!(request(&row(), 0).is_err());
        for key in [
            "owner",
            "index",
            "cost",
            "base",
            "title",
            "spent",
            "turn",
            "source",
            "ledger_active",
            "human",
        ] {
            let mut bad = row();
            bad.as_object_mut().unwrap().remove(key);
            assert!(request(&bad, 1).is_err(), "{key}");
        }
        assert!(candidates(&json!({"candidates":[row(),row()]})).is_err());
        let mut bad = row();
        bad["human"] = true.into();
        assert!(request(&bad, 1).is_err());
    }
}
