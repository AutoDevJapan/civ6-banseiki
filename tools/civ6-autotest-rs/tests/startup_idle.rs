use civ6_autotest_rs::{startup_idle::*, Error, ErrorKind, Result};
use serde_json::json;

fn proof() -> IdleProof {
    let adapters = vec![
        AdapterCoverage {
            id: "00000000:00001001".into(),
            nodes: 2,
            wddm_version: 3100,
        },
        AdapterCoverage {
            id: "00000000:00001002".into(),
            nodes: 1,
            wddm_version: 3100,
        },
    ];
    let engines = vec![
        EngineReading {
            pid: 94500,
            adapter: adapters[0].id.clone(),
            physical: 0,
            engine: 0,
            kind: "3D".into(),
            percent: 0.0,
            valid: true,
        },
        EngineReading {
            pid: 4,
            adapter: adapters[0].id.clone(),
            physical: 0,
            engine: 1,
            kind: "Copy".into(),
            percent: 0.0,
            valid: true,
        },
        EngineReading {
            pid: 800,
            adapter: adapters[1].id.clone(),
            physical: 0,
            engine: 0,
            kind: "VideoDecode".into(),
            percent: 0.0,
            valid: true,
        },
    ];
    IdleProof {
        source: IDLE_SOURCE.into(),
        status: "ok".into(),
        adapters,
        software_adapters: Vec::new(),
        adapters_stable: true,
        samples: (0..3)
            .map(|_| IdleSample {
                interval_ms: 1000,
                engines: engines.clone(),
            })
            .collect(),
        finished_unix: 100.0,
        detail: "native worker fixture".into(),
    }
}

#[test]
fn kernel_identified_software_adapter_is_accounted_for_without_ignoring_unknown_hardware() {
    let mut p = proof();
    let software = "00000000:0001cd78".to_string();
    p.software_adapters.push(software.clone());
    for sample in &mut p.samples {
        sample.engines.push(EngineReading {
            pid: 4,
            adapter: software.clone(),
            physical: 0,
            engine: 32,
            kind: "3D".into(),
            percent: 0.0,
            valid: true,
        });
    }
    assert_eq!(idle_refusal(&p), None);
    p.samples[1].engines.last_mut().unwrap().percent = 1.0;
    assert!(idle_refusal(&p).unwrap().contains("active"));
    p.samples[1].engines.last_mut().unwrap().percent = 0.0;
    p.software_adapters.clear();
    assert_eq!(idle_refusal(&p), Some("unrecognized GPU adapter or engine"));
    p.software_adapters.push(p.adapters[0].id.clone());
    assert!(idle_refusal(&p).unwrap().contains("overlaps"));
}
#[derive(Default)]
struct Input {
    keys: Vec<(u8, bool)>,
    checks: usize,
    lost_at: Option<usize>,
    pressed: Option<u8>,
    cancelled: bool,
    sleeps: usize,
    age: f64,
}
impl DisplayInput for Input {
    fn check_lock(&mut self) -> Result<()> {
        self.checks += 1;
        if self.lost_at == Some(self.checks) {
            Err(Error::input("lost lock"))
        } else {
            Ok(())
        }
    }
    fn check_cancel(&mut self) -> Result<()> {
        if self.cancelled {
            Err(Error::interrupted("cancelled"))
        } else {
            Ok(())
        }
    }
    fn unix_now(&mut self) -> f64 {
        100.0 + self.age
    }
    fn key_is_down(&mut self, key: u8) -> bool {
        self.pressed == Some(key)
    }
    fn key_event(&mut self, key: u8, up: bool) {
        self.keys.push((key, up));
    }
    fn sleep_100ms(&mut self) {
        self.sleeps += 1;
    }
}
#[test]
fn production_pdh_policy_is_reachable_and_uses_the_exact_original_shortcut() {
    let proof = proof();
    let round_trip: IdleProof =
        serde_json::from_value(serde_json::to_value(&proof).unwrap()).unwrap();
    assert_eq!(idle_refusal(&round_trip), None);
    let mut input = Input::default();
    let result = send_recovery_shortcut(&mut input, &round_trip).unwrap();
    assert_eq!(result["status"], "sent");
    assert_eq!(
        input.keys,
        vec![
            (0x5b, false),
            (0x11, false),
            (0x10, false),
            (0x42, false),
            (0x42, true),
            (0x10, true),
            (0x11, true),
            (0x5b, true)
        ]
    );
    assert_eq!(input.sleeps, 1);
    assert_eq!(input.checks, 2);
}
#[test]
fn busy_cuda_3d_compute_wsl_copy_and_even_owned_work_are_refused() {
    for (pid, kind) in [
        (94500, "3D"),
        (1161, "Compute"),
        (4, "3D"),
        (800, "Copy"),
        (11, "3D"),
    ] {
        let mut proof = proof();
        proof.samples[1].engines[0].pid = pid;
        proof.samples[1].engines[0].kind = kind.into();
        proof.samples[1].engines[0].percent = 1.0;
        let mut input = Input::default();
        assert_eq!(
            send_recovery_shortcut(&mut input, &proof).unwrap()["status"],
            "skipped"
        );
        assert!(input.keys.is_empty());
    }
}
#[test]
fn inactive_samples_with_missing_unknown_changed_or_invalid_coverage_are_refused() {
    for mode in 0..10 {
        let mut proof = proof();
        match mode {
            0 => proof.status = "unknown".into(),
            1 => proof.adapters_stable = false,
            2 => proof.samples.pop().map(|_| ()).unwrap(),
            3 => {
                proof.samples[0].engines.pop();
            }
            4 => proof.samples[0].engines[0].adapter = "unknown".into(),
            5 => proof.samples[0].engines[0].valid = false,
            6 => proof.samples[0].engines[0].percent = f64::NAN,
            7 => proof.samples[0].interval_ms = 999,
            8 => proof.adapters[0].wddm_version = 1300,
            _ => proof.samples[0].engines[0].physical = 1,
        };
        assert!(idle_refusal(&proof).is_some(), "mode {mode}");
        let mut input = Input::default();
        assert_eq!(
            send_recovery_shortcut(&mut input, &proof).unwrap()["status"],
            "skipped"
        );
        assert!(input.keys.is_empty());
    }
}
#[test]
fn summed_subthreshold_clients_cannot_hide_a_busy_engine() {
    let mut proof = proof();
    let mut extra = proof.samples[1].engines[0].clone();
    extra.pid = 12345;
    extra.percent = 0.6;
    proof.samples[1].engines[0].percent = 0.6;
    proof.samples[1].engines.push(extra);
    assert_eq!(
        idle_refusal(&proof),
        Some("aggregate GPU engine work is active")
    );
}
#[test]
fn lock_loss_at_input_modifiers_cancellation_or_stale_proof_never_send_input() {
    for mut input in [
        Input {
            lost_at: Some(2),
            ..Default::default()
        },
        Input {
            pressed: Some(0x11),
            ..Default::default()
        },
        Input {
            pressed: Some(0x5c),
            ..Default::default()
        },
        Input {
            cancelled: true,
            ..Default::default()
        },
        Input {
            age: 0.51,
            ..Default::default()
        },
    ] {
        let _ = send_recovery_shortcut(&mut input, &proof());
        assert!(input.keys.is_empty());
        assert_eq!(input.sleeps, 0);
    }
    let mut cancelled = Input {
        cancelled: true,
        ..Default::default()
    };
    assert_eq!(
        send_recovery_shortcut(&mut cancelled, &proof())
            .unwrap_err()
            .kind,
        ErrorKind::Interrupted
    );
}
#[test]
fn instance_parsing_does_not_filter_3d_or_cuda_or_wsl_processes() {
    let engine = engine_reading(
        "pid_94500_luid_0x00000000_0x00001001_phys_0_eng_0_engtype_3D",
        0.0,
        true,
    )
    .unwrap();
    assert_eq!(
        json!([engine.pid, engine.engine, engine.physical]),
        json!([94500, 0, 0])
    );
    assert_eq!(engine.adapter, "00000000:00001001");
    assert_eq!(engine.kind, "3D");
    assert!(engine_reading("_Total", 0.0, true).is_err());
    assert!(engine_reading("pid_bad", 0.0, true).is_err());
}
