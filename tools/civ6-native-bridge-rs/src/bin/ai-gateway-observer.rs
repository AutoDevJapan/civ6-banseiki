//! Finite candidate; the CLI never installs a hook or opens a game process.
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
#[path = "../ai_operation/capture.rs"]
mod capture;
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
#[path = "../ai_operation/gateway.rs"]
mod gateway;
#[path = "../ai_operation/pinned.rs"]
#[allow(dead_code)]
mod pinned;
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
#[path = "../ai_operation/protocol.rs"]
mod protocol;
fn main() {
    println!("{{\"candidate\":\"ai-stage2-four-site-gateway\",\"live_observations\":0,\"runtime_registered\":false,\"ai_acceptance\":false}}");
}
