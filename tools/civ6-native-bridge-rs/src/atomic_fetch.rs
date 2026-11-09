//! CPU policy shared by the Windows installer and portable policy tests.

pub(crate) fn vendor_supported(vendor: &[u8; 12]) -> bool {
    vendor == b"AuthenticAMD"
}

pub(crate) fn with_supported_cpu<T>(
    cpu_gate: bool,
    install: impl FnOnce() -> Result<T, &'static str>,
) -> Result<T, &'static str> {
    if !cpu_gate {
        return Err("runtime promotion requires AMD atomic operand fetch");
    }
    install()
}

pub(crate) fn cpu_vendor() -> [u8; 12] {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(target_arch = "x86")]
        use core::arch::x86::__cpuid;
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::__cpuid;
        let leaf = __cpuid(0);
        let mut vendor = [0u8; 12];
        vendor[..4].copy_from_slice(&leaf.ebx.to_le_bytes());
        vendor[4..8].copy_from_slice(&leaf.edx.to_le_bytes());
        vendor[8..].copy_from_slice(&leaf.ecx.to_le_bytes());
        vendor
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    *b"UnknownCPU  "
}

#[cfg(test)]
pub(crate) fn vendor_label(vendor: &[u8; 12]) -> String {
    vendor
        .iter()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() {
                char::from(*byte)
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_fetch_vendor_policy_accepts_amd_and_refuses_intel_and_unknown() {
        for (vendor, expected) in [
            (*b"AuthenticAMD", true),
            (*b"GenuineIntel", false),
            (*b"UnknownCPU  ", false),
            ([0; 12], false),
            (*b"authenticamd", false),
        ] {
            assert_eq!(
                vendor_supported(&vendor),
                expected,
                "atomic fetch vendor policy: {vendor:?}"
            );
        }
        println!(
            "ATOMIC_CPU vendor={} os={} arch={} required_vendor=AuthenticAMD",
            vendor_label(&cpu_vendor()),
            std::env::consts::OS,
            std::env::consts::ARCH,
        );
    }

    #[test]
    fn cpu_gate_false_never_enters_the_mutating_installer() {
        let mutations = std::cell::Cell::new(0);
        let install = || {
            mutations.set(mutations.get() + 1);
            Ok(42)
        };
        assert_eq!(
            with_supported_cpu(false, install),
            Err("runtime promotion requires AMD atomic operand fetch")
        );
        assert_eq!(mutations.get(), 0, "false CPU gate entered the installer");
        assert_eq!(with_supported_cpu(true, install), Ok(42));
        assert_eq!(
            mutations.get(),
            1,
            "true CPU gate did not install exactly once"
        );
    }
}
