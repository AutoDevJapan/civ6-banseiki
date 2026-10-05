//! Finite candidate; the CLI never installs a hook or opens a game process.
#[cfg(all(windows, target_arch = "x86_64"))]
#[allow(dead_code)]
#[path = "../ai_operation/gateway.rs"]
mod gateway;
fn main() {
    println!("{{\"candidate\":\"ai-stage2-four-site-gateway\",\"live_observations\":0,\"runtime_registered\":false,\"ai_acceptance\":false}}");
}
