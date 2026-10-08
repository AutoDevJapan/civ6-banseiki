//! Code-level replicas of the shipped oath Gameplay handler; no live LAN claim.
mod ages_support;
use ages_support::World;

fn world(local: i64) -> World {
    let w = World::new();
    w.turn(1);
    w.run(&format!(
        "Game.GetLocalPlayer=function() return {local} end"
    ));
    w.run(
        "local l=LEDGER(); l.reign=1; for _,id in ipairs({1,3}) do local h=l.houses['p'..id]; h.age='GOLDEN'; h.oathReign=1; h.oathChosen=nil; h.oaths={}; h.oathDue=99999 end; Game:SetProperty('JAC_AGES_LEDGER',l); P1={Reign=1,Oath1=assert(JAC_Oaths_Choices(l.houses.p1)[1]).OathType,PlayerID=3}; P3={Reign=1,Oath1=assert(JAC_Oaths_Choices(l.houses.p3)[1]).OathType,PlayerID=1}",
    );
    w
}

#[test]
fn replicated_oath_choices_ignore_local_player_payload_actor_and_request_order() {
    let host = world(1);
    let peer = world(3);
    host.run("CHOOSE_OATHS(1,P1); CHOOSE_OATHS(3,P3)");
    peer.run("CHOOSE_OATHS(3,P3); CHOOSE_OATHS(1,P1)");
    assert_eq!(host.snapshot(), peer.snapshot());
    assert!(host.truth("LEDGER().houses.p1.oaths[P1.Oath1] == true"));
    assert!(host.truth("LEDGER().houses.p3.oaths[P3.Oath1] == true"));
    assert_eq!(host.snapshot()["houses"]["p1"]["oathChosen"], 1i64);
    assert_eq!(host.snapshot()["houses"]["p3"]["oathChosen"], 1i64);
}

#[test]
fn invalid_and_foreign_oath_packets_do_not_change_either_replica() {
    let host = world(1);
    let peer = world(3);
    let original = host.snapshot();
    for packet in [
        "CHOOSE_OATHS(1,{Reign=0,Oath1=P1.Oath1})",
        "CHOOSE_OATHS(1,{Reign=1,Oath1=P3.Oath1})",
        "CHOOSE_OATHS(1,{Reign=1,Oath1=P1.Oath1,Oath2=P1.Oath1})",
        "CHOOSE_OATHS(1,{Reign=1,Oath1=P1.Oath1,Oath4='extra'})",
        "CHOOSE_OATHS(99,P1)",
        "CHOOSE_OATHS(1,nil)",
    ] {
        host.run(packet);
        peer.run(packet);
        assert_eq!(host.snapshot(), original, "{packet}");
        assert_eq!(peer.snapshot(), original, "{packet}");
    }
}

#[test]
fn save_reconstruction_and_rejoin_preserve_duplicate_choice_guard() {
    let host = world(1);
    host.run("CHOOSE_OATHS(1,P1); CHOOSE_OATHS(3,P3)");
    let saved = host.snapshot();
    let rejoined = world(3);
    // Strict saved values are rebuilt in a fresh VM, preserving Lua key types;
    // neither tables nor closures from the original endpoint can be shared.
    rejoined
        .lua
        .globals()
        .set("SAVED", saved.to_lua(&rejoined.lua))
        .unwrap();
    rejoined.run("Game:SetProperty('JAC_AGES_LEDGER',SAVED); RESET_HANDLERS()");
    rejoined.load_script();
    rejoined.run("CHOOSE_OATHS(3,P3); CHOOSE_OATHS(1,P1); CHOOSE_OATHS(1,P1)");
    assert_eq!(saved, rejoined.snapshot());
    host.run("CHOOSE_OATHS(1,P1); CHOOSE_OATHS(3,P3)");
    assert_eq!(host.snapshot(), rejoined.snapshot());
}

#[test]
fn simultaneous_replay_does_not_overwrite_later_knowledge_or_other_house() {
    let host = world(1);
    let peer = world(3);
    for w in [&host, &peer] {
        w.run("CHOOSE_OATHS(1,P1); LEDGER().houses.p1.oathKnowledge={tech=9,civic=8}");
    }
    let unchanged = host.snapshot();
    host.run("CHOOSE_OATHS(1,P1); CHOOSE_OATHS(1,P1)");
    peer.run("CHOOSE_OATHS(1,P1)");
    assert_eq!(unchanged, host.snapshot());
    assert_eq!(unchanged, peer.snapshot());
    assert!(peer.truth("LEDGER().houses.p3.oathChosen == nil"));
}
