use super::super::super::worlds;
use super::*;
fn unbound(s: &mut Ledger, record_window: bool) -> Lease {
    let lease = s.birth(1, Some(2), 30, 2).unwrap();
    if record_window {
        s.record_birth_window(lease, worlds::fixture_first_birth_window());
    }
    s.factory(lease, 7, 30, 40, 50).unwrap();
    s.published(lease, 7).unwrap();
    lease
}
fn world(serial: u64, epoch: u64) -> WorldLease {
    WorldLease {
        serial,
        epoch,
        context: 60,
        game: 70,
        world: 80,
    }
}
#[test]
fn initial_binding_needs_the_captured_birth_window_and_native_world() {
    let mut s = Ledger::default();
    let lease = unbound(&mut s, false);
    assert!(s.bootstrap_target(30, 40).is_none());
    assert_eq!(s.world(lease), Err(Error::Unbound));
    let mut s = Ledger::default();
    let lease = unbound(&mut s, true);
    let target = s.bootstrap_target(30, 40).unwrap();
    assert_eq!(
        s.bind_bootstrap_world(target, world(2, 2)),
        Err(Error::Stale)
    );
    assert_eq!(s.world(lease), Err(Error::Unbound));
    s.bind_bootstrap_world(target, world(1, 1)).unwrap();
    assert_eq!(
        s.world(lease),
        Ok(WorldReceipt {
            binding_id: 1,
            epoch: 1
        })
    );
    assert!(s.bootstrap_target(30, 40).is_none());
    assert_eq!(
        s.bind_bootstrap_world(target, world(1, 1)),
        Err(Error::Stale)
    );
    let transport = s.publish_transport(lease, 900, 90, 100, 110).unwrap();
    let message = s.reserve_input(lease, transport).unwrap().unwrap();
    assert_eq!(
        s.message_world(message),
        Ok(WorldReceipt {
            binding_id: 1,
            epoch: 1
        })
    );
}
#[test]
fn binding_a_world_does_not_reissue_a_pre_world_transport_ticket() {
    let mut s = Ledger::default();
    let lease = unbound(&mut s, true);
    let transport = s.publish_transport(lease, 900, 90, 100, 110).unwrap();
    assert_eq!(s.reserve_input(lease, transport), Ok(None));
    let target = s.bootstrap_target(30, 40).unwrap();
    s.bind_bootstrap_world(target, world(1, 1)).unwrap();
    assert_eq!(s.reserve_input(lease, transport), Ok(None));
    assert!(s.bootstrap_target(31, 40).is_none());
    assert!(s.bootstrap_target(30, 41).is_none());
}

#[test]
fn initial_binding_rejects_closed_aliases_and_changed_transport_incarnations() {
    let mut s = Ledger::default();
    let old = unbound(&mut s, true);
    let target = s.bootstrap_target(30, 40).unwrap();
    let transport = s.publish_transport(old, 900, 90, 100, 110).unwrap();
    assert!(s.bootstrap_packet_matches(old, 900));
    assert!(!s.bootstrap_packet_matches(old, 901));
    s.transport_disconnect_begin(old).unwrap();
    assert!(!s.bootstrap_packet_matches(old, 900));
    s.transport_join_result(old, transport, 110, 1).unwrap();
    s.transport_disconnect_end(old).unwrap();
    assert!(!s.bootstrap_packet_matches(old, 900));
    s.publish_transport(old, 901, 90, 100, 110).unwrap();
    assert!(s.bootstrap_packet_matches(old, 901));
    assert!(!s.bootstrap_packet_matches(old, 900));
    // Closing the Session invalidates the saved proposal, even before its
    // native addresses can be reused for a later incarnation.
    s.close(old).unwrap();
    assert!(s.bootstrap_target(30, 40).is_none());
    assert_eq!(
        s.bind_bootstrap_world(target, world(1, 1)),
        Err(Error::Unbound)
    );
    assert_eq!(s.world(old), Err(Error::Unbound));
}

#[test]
fn alternate_or_unobserved_session_birth_never_proposes_an_initial_world() {
    for (lifetime, kind) in [(None, 2), (Some(2), 4)] {
        let mut s = Ledger::default();
        let lease = s.birth(1, lifetime, 30, kind).unwrap();
        s.record_birth_window(lease, worlds::fixture_first_birth_window());
        s.factory(lease, 7, 30, 40, 50).unwrap();
        s.published(lease, 7).unwrap();
        assert!(s.bootstrap_target(30, 40).is_none());
        assert!(s.records.get(&lease.0).unwrap().birth_window.is_none());
    }
}
