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
#[test]
fn pre_world_input_transport_stays_owned_until_its_actual_storage_is_released() {
    let mut ledger = Ledger::default();
    let lease = unbound(&mut ledger, true);
    let transport = ledger.publish_transport(lease, 900, 90, 100, 110).unwrap();
    assert_eq!(ledger.reserve_input(lease, transport), Ok(None));
    let hold = ledger.retain_input_transport(lease, transport).unwrap();
    let target = ledger.bootstrap_target(30, 40).unwrap();
    ledger.bind_bootstrap_world(target, world(1, 1)).unwrap();
    let bound = WorldReceipt {
        binding_id: 1,
        epoch: 1,
    };
    assert_eq!(ledger.retire_world(bound), Err(Error::Busy));
    assert_eq!(ledger.drained(bound), Err(Error::Busy));
    assert_eq!(ledger.reserve(lease), Err(Error::Closed));
    ledger.release_input_transport(hold).unwrap();
    assert_eq!(ledger.release_input_transport(hold), Err(Error::Unknown));
    ledger.retire_world(bound).unwrap();
    assert_eq!(ledger.reserve_input(lease, transport), Ok(None));
    let new = WorldReceipt {
        binding_id: 2,
        epoch: 2,
    };
    ledger.loaded_world(lease, Some(bound), new).unwrap();
    let later = ledger.retain_input_transport(lease, transport).unwrap();
    assert!(ledger.has_input_holds(bound));
    assert!(
        !ledger.has_input_holds(new),
        "old transport lifetime moved into the latest World"
    );
    ledger.release_input_transport(later).unwrap();
}
#[test]
fn unbound_old_transport_storage_denies_completion_without_adopting_the_new_world() {
    let mut ledger = Ledger::default();
    let lease = unbound(&mut ledger, true);
    let old = ledger.publish_transport(lease, 900, 90, 100, 110).unwrap();
    let hold = ledger.retain_input_transport(lease, old).unwrap();
    ledger.transport_disconnect_begin(lease).unwrap();
    ledger
        .transport_join_result(lease, old, old.thread, 1)
        .unwrap();
    ledger.transport_disconnect_end(lease).unwrap();
    ledger.publish_transport(lease, 901, 90, 100, 110).unwrap();
    let target = ledger.bootstrap_target(30, 40).unwrap();
    ledger.bind_bootstrap_world(target, world(1, 1)).unwrap();
    assert_eq!(ledger.input_holds.get(&hold.0).unwrap().world, None);
    let world = WorldReceipt {
        binding_id: 1,
        epoch: 1,
    };
    assert_eq!(ledger.retire_world(world), Err(Error::Busy));
    ledger.release_input_transport(hold).unwrap();
    ledger.retire_world(world).unwrap();
}
#[test]
fn unbound_input_from_a_removed_session_remains_a_completion_obligation() {
    let mut ledger = Ledger::default();
    let old = unbound(&mut ledger, true);
    let transport = ledger.publish_transport(old, 900, 90, 100, 110).unwrap();
    let hold = ledger.retain_input_transport(old, transport).unwrap();
    ledger.terminal_remove_begin(1, 2, 7, 40).unwrap().unwrap();
    ledger
        .transport_join_result(old, transport, 110, 1)
        .unwrap();
    ledger.transport_disconnect_end(old).unwrap();
    ledger.terminal_remove_end(old).unwrap();

    // Exact native addresses may be reused after the original removal. The
    // surviving input is still owned by the earlier Session/transport.
    let fresh = unbound(&mut ledger, true);
    assert_ne!(fresh, old);
    let target = ledger.bootstrap_target(30, 40).unwrap();
    ledger.bind_bootstrap_world(target, world(1, 1)).unwrap();
    let current = WorldReceipt {
        binding_id: 1,
        epoch: 1,
    };
    assert_eq!(ledger.input_holds[&hold.0].lease, old);
    assert_eq!(ledger.input_holds[&hold.0].world, None);
    assert_eq!(ledger.retire_world(current), Err(Error::Busy));
    assert_eq!(ledger.drained(current), Err(Error::Busy));
    assert_eq!(ledger.input_holds[&hold.0].world, None);
    ledger.release_input_transport(hold).unwrap();
    assert_eq!(ledger.release_input_transport(hold), Err(Error::Unknown));
    ledger.retire_world(current).unwrap();
    let next = WorldReceipt {
        binding_id: 2,
        epoch: 2,
    };
    ledger.loaded_world(fresh, Some(current), next).unwrap();
    ledger.publish_transport(fresh, 901, 90, 100, 110).unwrap();
    let packet = ledger.reserve(fresh).unwrap();
    assert_eq!(ledger.message_world(packet), Ok(next));
    ledger.cancel_before_dispatch(packet).unwrap();
}
#[test]
fn receive_obligations_keep_exact_sources_and_require_every_return() {
    let mut ledger = Ledger::default();
    let lease = unbound(&mut ledger, true);
    let transport = ledger.publish_transport(lease, 900, 90, 100, 110).unwrap();
    let source = Some((lease, transport));
    let first = ledger.retain_receive(source).unwrap();
    let second = ledger.retain_receive(None).unwrap();
    let target = ledger.bootstrap_target(30, 40).unwrap();
    ledger.bind_bootstrap_world(target, world(1, 1)).unwrap();
    let bound = WorldReceipt {
        binding_id: 1,
        epoch: 1,
    };
    assert_eq!(ledger.retire_world(bound), Err(Error::Busy));
    assert_eq!(ledger.release_receive(first, None), Err(Error::Stale));
    assert_eq!(ledger.receive_holds[&first.0], source);
    ledger.release_receive(first, source).unwrap();
    assert_eq!(ledger.release_receive(first, source), Err(Error::Unknown));
    assert_eq!(ledger.retire_world(bound), Err(Error::Busy));
    ledger.release_receive(second, None).unwrap();
    ledger.retire_world(bound).unwrap();
    let next = WorldReceipt {
        binding_id: 2,
        epoch: 2,
    };
    ledger.loaded_world(lease, Some(bound), next).unwrap();
    assert_eq!(ledger.world(lease), Ok(next));
    assert!(ledger.receive_holds.is_empty());
}
#[test]
fn repeated_reset_accepts_only_new_unpublished_bindings_and_retains_older_packets() {
    let mut ledger = Ledger::default();
    let lease = unbound(&mut ledger, true);
    let target = ledger.bootstrap_target(30, 40).unwrap();
    ledger.bind_bootstrap_world(target, world(1, 1)).unwrap();
    let original = ledger.publish_transport(lease, 900, 90, 100, 110).unwrap();
    let packet = ledger.reserve_input(lease, original).unwrap().unwrap();
    ledger.native_reset_begin(lease, original).unwrap();
    ledger.native_reset_returned(lease, original).unwrap();
    assert_eq!(
        ledger.drained(WorldReceipt {
            binding_id: 1,
            epoch: 1
        }),
        Err(Error::Unresolved),
        "reset return alone must retain the World gap, before any packet settles"
    );
    let other = ledger.birth(2, Some(3), 31, 2).unwrap();
    ledger.factory(other, 8, 31, 41, 51).unwrap();
    ledger.published(other, 8).unwrap();
    ledger.publish_transport(other, 950, 190, 200, 210).unwrap();
    for invalid in [
        Transport {
            incarnation: 0,
            ..original
        },
        Transport {
            incarnation: 899,
            ..original
        },
        original,
        Transport {
            incarnation: 901,
            pointer: 91,
            ..original
        },
        Transport {
            incarnation: 950,
            ..original
        },
    ] {
        assert_eq!(ledger.native_reset_begin(lease, invalid), Err(Error::Stale));
        assert!(
            matches!(ledger.records[&lease.0].native_reset, Some(NativeReset::Returned(old)) if old == original)
        );
    }
    let pending = Transport {
        incarnation: 901,
        thread: 0,
        ..original
    };
    ledger.native_reset_begin(lease, pending).unwrap();
    assert_eq!(
        ledger.publish_transport(lease, 902, 90, 100, 110),
        Err(Error::Stale)
    );
    assert_eq!(
        ledger.native_reset_begin(
            lease,
            Transport {
                incarnation: 902,
                ..pending
            }
        ),
        Err(Error::Stale)
    );
    assert_eq!(
        ledger.native_reset_returned(lease, original),
        Err(Error::Stale)
    );
    ledger.native_reset_returned(lease, pending).unwrap();
    assert!(!ledger.records[&lease.0].transport_joined);
    assert!(!ledger.records[&lease.0].joined);
    let current = ledger.publish_transport(lease, 902, 90, 100, 110).unwrap();
    assert_eq!(ledger.transport(lease), Ok(current));
    assert_eq!(
        ledger.message_world(packet),
        Ok(WorldReceipt {
            binding_id: 1,
            epoch: 1
        })
    );
    assert_eq!(ledger.take_reset_input(packet, 51, 7), Err(Error::Stale));
    assert_eq!(ledger.take_reset_input(packet, 50, 7), Ok(Some(lease)));
    assert_eq!(
        ledger.drained(WorldReceipt {
            binding_id: 1,
            epoch: 1
        }),
        Err(Error::Unresolved)
    );
}
fn world(serial: u64, epoch: u64) -> WorldLease {
    WorldLease {
        serial,
        epoch,
        context: 60,
        game: 70,
        world: 80,
        observation: 0,
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
