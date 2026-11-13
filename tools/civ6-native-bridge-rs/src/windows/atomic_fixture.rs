//! Evidence for CPU-conditional, process-owned gateway fixtures.

pub(super) struct Executed(&'static str);

pub(super) fn require(name: &'static str) -> Option<Executed> {
    require_for_vendor(name, &crate::atomic_fetch::cpu_vendor(), |line| {
        println!("{line}")
    })
}

fn require_for_vendor(
    name: &'static str,
    vendor: &[u8; 12],
    report: impl FnOnce(String),
) -> Option<Executed> {
    if crate::atomic_fetch::vendor_supported(vendor) {
        Some(Executed(name))
    } else {
        report(format!(
            "ATOMIC_FIXTURE status=NOT_EXECUTED cpu_vendor={} test={name}; NOT EXECUTED: requires Windows x86_64 AuthenticAMD atomic instruction fetch",
            crate::atomic_fetch::vendor_label(vendor),
        ));
        None
    }
}

impl Drop for Executed {
    fn drop(&mut self) {
        // A failed assertion must never become evidence of fixture execution.
        // Expected-abort tests retain this guard in the supervising parent,
        // which validates the child's exact refusal before dropping it.
        if !std::thread::panicking() {
            println!(
                "ATOMIC_FIXTURE status=EXECUTED cpu_vendor={} test={}",
                crate::atomic_fetch::vendor_label(&crate::atomic_fetch::cpu_vendor()),
                self.0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_fixture_reports_not_executed_with_vendor_and_condition() {
        for vendor in [*b"GenuineIntel", *b"UnknownCPU  "] {
            let mut line = String::new();
            let fixture =
                require_for_vendor("unsupported_fixture", &vendor, |report| line = report);
            assert!(fixture.is_none());
            assert!(line.contains("status=NOT_EXECUTED"));
            assert!(line.contains("NOT EXECUTED: requires Windows x86_64 AuthenticAMD"));
            assert!(line.contains(&format!(
                "cpu_vendor={}",
                crate::atomic_fetch::vendor_label(&vendor)
            )));
        }
    }
}
