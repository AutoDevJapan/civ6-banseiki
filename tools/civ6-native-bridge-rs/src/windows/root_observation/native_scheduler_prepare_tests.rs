//! Explicit licensed-image preparation without executing native game bodies.
use super::super::super::primary_image::Image;
use super::*;

#[test]
#[ignore = "explicit licensed local primary-image owned preparation"]
fn primary_early_host_prepare_after_existing_owners_publishes_scheduler_and_restores() {
    if !child("early_prepare::primary_early_host_prepare_after_existing_owners_publishes_scheduler_and_restores") {
        return;
    }
    let image = Image::new(GAME_EXE, civ6_bootstrap_rs::EXE_SHA);
    assert!(FRAMES.get().is_none());
    let start = Instant::now();
    let prepare = unsafe { super::super::super::prepare_host(image.base) };
    println!(
        "OWNED_PRIMARY_HOST_PREPARE result={prepare:?} elapsed={:?}",
        start.elapsed()
    );
    let published = HOOKS.lock().unwrap().len();
    let restore = unsafe { super::super::super::restore() };
    println!("OWNED_PRIMARY_HOST_RESTORE result={restore:?} scheduler_owners={published}");
    prepare.unwrap();
    assert_eq!(published, 8);
    restore.unwrap();
    assert!(FRAMES.get().is_none());
    assert!(!FAILED.load(Ordering::Acquire));
    for (rva, prefix, _) in SITES {
        assert_eq!(copy_current::<8>((image.base + rva) as u64), Some(prefix));
    }
}
