use super::*;
use civ6_native_bridge_rs::governor_payment::PROMOTION_WORDS;

const EPOCH: Epoch = Epoch {
    pid: 42,
    created: 1234,
    generation: 1,
};

// Scalar synthetic replay fixtures, not invented Lua/Core APIs. Definitions
// are named from checked-in SQL; their numeric indices are explicitly synthetic.
fn fixture(target: &str, paid: u8) -> Vec<Person<'static>> {
    let sql = include_str!("../../../../poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona.sql");
    let rows = sql
        .split("INSERT INTO JAC_PersonaDesigns VALUES")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let mut names: Vec<_> = rows
        .lines()
        .filter_map(|line| line.trim().strip_prefix("('"))
        .map(|line| line.split('\'').next().unwrap())
        .collect();
    names.sort();
    names
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            let suffix = name.strip_prefix("LEADER_JAC_").unwrap();
            let base = leak(format!("GOVERNOR_PROMOTION_JAC_PERSONA_{suffix}_BASE"));
            let cost = match name {
                DATE => 2,
                UESUGI => 3,
                _ => 1,
            };
            let def = PaymentDefinition {
                governor_index: i as u32 + 7,
                cost,
                base: i as u32 * 3,
                titles: [
                    (cost >= 2).then_some(i as u32 * 3 + 1),
                    (cost == 3).then_some(i as u32 * 3 + 2),
                ],
            };
            let selected = name == target;
            let mut native = PromotionSnapshot {
                owner: 1,
                governor_index: def.governor_index,
                count: 100,
                owned: [0; PROMOTION_WORDS],
            };
            for index in std::iter::once(def.base)
                .chain(def.titles.into_iter().flatten())
                .take(paid as usize)
            {
                native.owned[index as usize / 64] |= 1 << (index % 64);
            }
            Person {
                leader: name,
                governor_type: leak(format!("GOVERNOR_JAC_PERSONA_{suffix}")),
                base_type: base,
                title_types: [
                    (cost >= 2).then(|| leak(format!("{base}_TITLE_1"))),
                    (cost == 3).then(|| leak(format!("{base}_TITLE_2"))),
                ],
                definition: def,
                owner: selected.then_some(1),
                stage: if selected {
                    Stage::Evolving
                } else {
                    Stage::GreatPerson
                },
                great_person_present: selected,
                native: (selected && paid > 0).then_some(native),
            }
        })
        .collect()
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}
fn frame<'a>(people: &'a [Person<'a>], sequence: u64, spent: u32) -> Frame<'a> {
    Frame {
        epoch: EPOCH,
        sequence,
        turn: 7,
        owner: 1,
        earned: 3,
        spent,
        people,
    }
}
fn replay() -> Replay {
    Replay::new(EPOCH, 1, None).unwrap()
}
fn own<'a>(people: &'a mut [Person<'static>], name: &str) -> &'a mut Person<'static> {
    people.iter_mut().find(|p| p.leader == name).unwrap()
}
fn proposal(d: Decision) -> Candidate {
    match d {
        Decision::Propose(c) => c,
        _ => panic!("expected exactly one proposal"),
    }
}

#[test]
fn date_one_title_then_no_duplicate_even_if_send_was_reported_twice() {
    let before = fixture(DATE, 1);
    let mut r = replay();
    let c = proposal(r.observe(&frame(&before, 1, 1)).unwrap());
    let p = before.iter().find(|p| p.leader == DATE).unwrap();
    assert_eq!(c.promotion_index, p.definition.titles[0].unwrap());
    assert_eq!(c.governor_index, p.definition.governor_index);
    assert_eq!(r.observe(&frame(&before, 2, 1)), Ok(Decision::AwaitResult));
    let after = fixture(DATE, 2);
    assert_eq!(
        r.observe(&frame(&after, 3, 2)),
        Ok(Decision::ObservedOnePayment)
    );
    assert_eq!(
        r.observe(&frame(&after, 4, 2)),
        Ok(Decision::NoAdditionalTitle)
    );
}

#[test]
fn older_payment_snapshot_with_new_sequence_cannot_reissue_acknowledged_title() {
    let before = fixture(DATE, 1);
    let after = fixture(DATE, 2);
    let mut r = replay();
    proposal(r.observe(&frame(&before, 1, 1)).unwrap());
    assert_eq!(
        r.observe(&frame(&after, 2, 2)),
        Ok(Decision::ObservedOnePayment)
    );
    assert_eq!(
        r.observe(&frame(&before, 3, 1)),
        Err(Failure::AmbiguousResult)
    );
}

#[test]
fn uesugi_two_distinct_steps_and_reload_reconstructs_remaining_one() {
    let mut r = replay();
    let one = fixture(UESUGI, 1);
    let c1 = proposal(r.observe(&frame(&one, 1, 1)).unwrap());
    let two = fixture(UESUGI, 2);
    assert_eq!(
        r.observe(&frame(&two, 2, 2)),
        Ok(Decision::ObservedOnePayment)
    );
    assert_eq!(
        r.observe(&frame(&two, 3, 2)),
        Ok(Decision::AlreadyProposedThisTurn)
    );
    let mut next_turn = frame(&two, 4, 2);
    next_turn.turn = 8;
    let c2 = proposal(r.observe(&next_turn).unwrap());
    assert_eq!((c1.turn, c2.turn), (7, 8));
    assert_ne!(c1.promotion_index, c2.promotion_index);
    let three = fixture(UESUGI, 3);
    assert_eq!(
        r.observe(&Frame {
            turn: 8,
            ..frame(&three, 5, 3)
        }),
        Ok(Decision::ObservedOnePayment)
    );
    assert_eq!(
        r.observe(&Frame {
            turn: 8,
            ..frame(&three, 6, 3)
        }),
        Ok(Decision::NoAdditionalTitle)
    );
    let new_epoch = Epoch {
        generation: 2,
        ..EPOCH
    };
    let mut reloaded = Replay::new(new_epoch, 1, Some(c1.turn)).unwrap();
    let mut f = frame(&two, 1, 2);
    f.epoch = new_epoch;
    assert_eq!(reloaded.observe(&f), Ok(Decision::AlreadyProposedThisTurn));
    f.sequence = 2;
    f.turn = 8;
    let resumed = proposal(reloaded.observe(&f).unwrap());
    assert_eq!(resumed.promotion_index, c2.promotion_index);
    assert_eq!(resumed.previous_paid, 2);
}

#[test]
fn repeated_callback_cannot_propose_a_different_person_in_the_same_turn() {
    let mut before = fixture(DATE, 1);
    *own(&mut before, UESUGI) = fixture(UESUGI, 1)
        .into_iter()
        .find(|p| p.leader == UESUGI)
        .unwrap();
    let mut r = replay();
    let mut f = frame(&before, 1, 2);
    f.earned = 5;
    assert_eq!(
        proposal(r.observe(&f).unwrap()).governor_index,
        own(&mut before, DATE).definition.governor_index
    );
    *own(&mut before, DATE) = fixture(DATE, 2)
        .into_iter()
        .find(|p| p.leader == DATE)
        .unwrap();
    let mut f = frame(&before, 2, 3);
    f.earned = 5;
    assert_eq!(r.observe(&f), Ok(Decision::ObservedOnePayment));
    for sequence in 3..20 {
        f.sequence = sequence;
        assert_eq!(r.observe(&f), Ok(Decision::AlreadyProposedThisTurn));
    }
    f.turn = 8;
    f.sequence = 20;
    let c = proposal(r.observe(&f).unwrap());
    assert_eq!(
        c.governor_index,
        before
            .iter()
            .find(|p| p.leader == UESUGI)
            .unwrap()
            .definition
            .governor_index
    );
}

#[test]
fn pending_attempt_survives_new_turn_without_reissue_and_turn_regression_poisoned() {
    let before = fixture(UESUGI, 1);
    let mut r = replay();
    proposal(r.observe(&frame(&before, 1, 1)).unwrap());
    let f = Frame {
        turn: 8,
        ..frame(&before, 2, 1)
    };
    assert_eq!(r.observe(&f), Ok(Decision::AwaitResult));
    assert_eq!(r.observe(&frame(&before, 3, 1)), Err(Failure::Order));
    assert_eq!(
        r.observe(&Frame {
            turn: 9,
            ..frame(&before, 4, 1)
        }),
        Err(Failure::AmbiguousResult)
    );
}

#[test]
fn restored_turn_mark_blocks_same_turn_and_rejects_earlier_saved_turn() {
    let two = fixture(UESUGI, 2);
    let mut r = Replay::new(EPOCH, 1, Some(7)).unwrap();
    assert_eq!(
        r.observe(&frame(&two, 1, 2)),
        Ok(Decision::AlreadyProposedThisTurn)
    );
    assert_eq!(
        r.observe(&Frame {
            turn: 6,
            ..frame(&two, 2, 2)
        }),
        Err(Failure::Order)
    );
}

#[test]
fn wrong_owner_index_missing_base_and_nonprefix_flags_fail() {
    for case in 0..4 {
        let mut people = fixture(UESUGI, 1);
        let p = own(&mut people, UESUGI);
        let n = p.native.as_mut().unwrap();
        match case {
            0 => n.owner = 2,
            1 => n.governor_index += 1,
            2 => n.owned[p.definition.base as usize / 64] &= !(1 << (p.definition.base % 64)),
            _ => {
                let ix = p.definition.titles[1].unwrap();
                n.owned[ix as usize / 64] |= 1 << (ix % 64);
            }
        }
        assert!(
            replay().observe(&frame(&people, 1, 1)).is_err(),
            "case {case}"
        );
    }
}

#[test]
fn aggregate_spent_and_ui_send_are_not_personal_payment() {
    let before = fixture(DATE, 1);
    let mut r = replay();
    proposal(r.observe(&frame(&before, 1, 1)).unwrap());
    assert_eq!(r.observe(&frame(&before, 2, 2)), Err(Failure::Reservation));
    // An ambiguous attempt cannot be made eligible by a later plausible row.
    let after = fixture(DATE, 2);
    assert_eq!(
        r.observe(&frame(&after, 3, 2)),
        Err(Failure::AmbiguousResult)
    );
}

#[test]
fn another_person_payment_cannot_complete_pending_request() {
    let mut before = fixture(DATE, 1);
    let uesugi = fixture(UESUGI, 1);
    let u = uesugi.into_iter().find(|p| p.leader == UESUGI).unwrap();
    *own(&mut before, UESUGI) = u;
    let mut r = replay();
    let mut f = frame(&before, 1, 2);
    f.earned = 5;
    proposal(r.observe(&f).unwrap());
    let u = fixture(UESUGI, 2)
        .into_iter()
        .find(|p| p.leader == UESUGI)
        .unwrap();
    *own(&mut before, UESUGI) = u;
    let mut after = frame(&before, 2, 3);
    after.earned = 5;
    assert_eq!(r.observe(&after), Err(Failure::AmbiguousResult));
}

#[test]
fn full_other_reservations_and_catalog_completeness_are_required() {
    let mut p = fixture(DATE, 1);
    own(&mut p, UESUGI).owner = Some(1);
    own(&mut p, UESUGI).great_person_present = true;
    own(&mut p, UESUGI).stage = Stage::Evolving;
    assert_eq!(
        replay().observe(&frame(&p, 1, 1)),
        Err(Failure::Reservation)
    );
    p.pop();
    assert_eq!(replay().observe(&frame(&p, 1, 1)), Err(Failure::Catalog));
}

#[test]
fn same_size_catalog_with_replaced_person_is_rejected() {
    let mut p = fixture(DATE, 1);
    let replacement = own(&mut p, DATE);
    replacement.leader = "LEADER_JAC_FAKE";
    replacement.governor_type = "GOVERNOR_JAC_PERSONA_FAKE";
    replacement.base_type = "GOVERNOR_PROMOTION_JAC_PERSONA_FAKE_BASE";
    replacement.title_types = [
        Some("GOVERNOR_PROMOTION_JAC_PERSONA_FAKE_BASE_TITLE_1"),
        None,
    ];
    p.sort_by_key(|p| p.leader);
    assert_eq!(replay().observe(&frame(&p, 1, 1)), Err(Failure::Catalog));
}

#[test]
fn pid_generation_sequence_and_catalog_cannot_be_rebound() {
    for case in 0..5 {
        let mut p = fixture(DATE, 1);
        let mut r = replay();
        proposal(r.observe(&frame(&p, 1, 1)).unwrap());
        if case == 4 {
            own(&mut p, DATE).definition.governor_index = 200;
        }
        let mut f = frame(&p, 2, 1);
        match case {
            0 => f.epoch.pid += 1,
            1 => f.epoch.created += 1,
            2 => f.epoch.generation += 1,
            3 => f.sequence = 1,
            _ => {}
        }
        assert!(r.observe(&f).is_err(), "case {case}");
    }
}

#[test]
fn skips_appointment_and_rejects_wrong_title_prefix_or_cost() {
    let p = fixture(DATE, 0);
    assert_eq!(
        replay().observe(&frame(&p, 1, 0)),
        Ok(Decision::NoAdditionalTitle)
    );
    let mut p = fixture(DATE, 1);
    own(&mut p, DATE).title_types[0] = Some("GOVERNOR_PROMOTION_JAC_PERSONA_OTHER_BASE_TITLE_1");
    assert_eq!(replay().observe(&frame(&p, 1, 1)), Err(Failure::Catalog));
}

#[test]
fn zero_cost_missing_title_and_false_full_payment_never_authorize() {
    for case in 0..3 {
        let mut p = fixture(UESUGI, 1);
        let person = own(&mut p, UESUGI);
        match case {
            0 => person.definition.cost = 0,
            1 => person.definition.titles[1] = None,
            _ => person.stage = Stage::Governor,
        }
        assert!(
            replay().observe(&frame(&p, 1, 1)).is_err(),
            "missing payment case {case}"
        );
    }
}

#[test]
fn extra_nonpayment_flag_or_two_titles_at_once_is_ambiguous() {
    for two_at_once in [false, true] {
        let before = fixture(UESUGI, 1);
        let mut r = replay();
        proposal(r.observe(&frame(&before, 1, 1)).unwrap());
        let mut after = fixture(UESUGI, if two_at_once { 3 } else { 2 });
        if !two_at_once {
            own(&mut after, UESUGI).native.as_mut().unwrap().owned[1] |= 1 << 30;
        }
        assert_eq!(
            r.observe(&frame(&after, 2, if two_at_once { 3 } else { 2 })),
            Err(Failure::AmbiguousResult)
        );
    }
}
