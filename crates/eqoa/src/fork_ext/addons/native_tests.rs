use super::host::Host;

#[test]
fn native_capabilities_are_probeable_and_stubs_diagnose_even_when_caught() {
    let h = Host::new().unwrap();
    h.execute(r#"
        assert(eqoa.api == 'eqoa/1')
        assert(eqoa.capabilities.has('eqoa.capabilities.get'))
        assert(eqoa.capabilities.get('eqoa.inventory.get').status == 'not_yet')
        assert(eqoa.capabilities.get('eqoa.no_such_symbol').status == 'unknown')
        local v,e = eqoa.inventory.get()
        assert(v == nil and e.code == 'not_yet')
        local ok = pcall(function() return eqoa.player.misspelled end)
        assert(ok)
    "#).unwrap();
    let logs = h.drain_logs().unwrap();
    assert!(logs.iter().any(|s| s.contains("eqoa.inventory.get")));
    assert!(logs.iter().any(|s| s.contains("eqoa.player.misspelled")));
}

#[test]
fn native_pregame_getters_do_not_fabricate_world_or_hud_claims() {
    let h = Host::new().unwrap();
    h.execute(r#"
        assert(eqoa.client.get().in_world == false)
        local p,e = eqoa.player.get()
        assert(p == nil and e.code == 'not_in_world')
        local claim,e = eqoa.hud.hide('health')
        assert(claim == nil and e.code == 'not_in_world')
        assert(#eqoa.hud.hidden_by('all') == 0)
        assert(eqoa.input.get().pad == nil)
        local ok = pcall(function() eqoa.api = 'changed' end)
        assert(not ok)
    "#).unwrap();
}
