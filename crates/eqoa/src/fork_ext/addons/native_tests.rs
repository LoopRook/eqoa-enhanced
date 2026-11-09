use super::host::Host;

#[test]
fn native_capabilities_are_probeable_and_stubs_diagnose_even_when_caught() {
    let h = Host::new().unwrap();
    h.execute(
        r#"
        assert(eqoa.api == 'eqoa/1')
        assert(eqoa.capabilities.has('eqoa.capabilities.get'))
        assert(eqoa.capabilities.get('eqoa.inventory.get').status == 'not_yet')
        assert(eqoa.capabilities.get('eqoa.no_such_symbol').status == 'unknown')
        local v,e = eqoa.inventory.get()
        assert(v == nil and e.code == 'not_yet')
        local ok = pcall(function() return eqoa.player.misspelled end)
        assert(ok)
    "#,
    )
    .unwrap();
    let logs = h.drain_logs().unwrap();
    assert!(logs.iter().any(|s| s.contains("eqoa.inventory.get")));
    assert!(logs.iter().any(|s| s.contains("eqoa.player.misspelled")));
}

#[test]
fn native_pregame_getters_do_not_fabricate_world_or_hud_claims() {
    let h = Host::new().unwrap();
    h.execute(
        r#"
        assert(eqoa.client.get().in_world == false)
        local p,e = eqoa.player.get()
        assert(p == nil and e.code == 'not_in_world')
        local claim,e = eqoa.hud.hide('health')
        assert(claim == nil and e.code == 'not_in_world')
        assert(#eqoa.hud.hidden_by('all') == 0)
        assert(eqoa.input.get().pad == nil)
        local ok = pcall(function() eqoa.api = 'changed' end)
        assert(not ok)
    "#,
    )
    .unwrap();
}

#[test]
fn native_events_allow_removal_during_dispatch_and_report_real_overflow() {
    use super::{
        native_core::{CORE, Core},
        native_events,
    };
    CORE.with(|c| *c.borrow_mut() = Core::default());
    let h = Host::new().unwrap();
    h.execute("calls={};eqoa.events.on('chat.message',function(e) calls[#calls+1]='first';eqoa.events.off(second) end);second=eqoa.events.on('chat.message',function() calls[#calls+1]='second' end);eqoa.events.once('chat.message',function() calls[#calls+1]='once' end);eqoa.events.on('events.dropped',function(e) dropped=e.data.count end);eqoa.events.after(0,function() timer_called=true end)").unwrap();
    super::native_core::observe_text("one", [1.0; 4]);
    native_events::dispatch(&h).unwrap();
    h.execute("assert(table.concat(calls,',')=='first,once');assert(timer_called)")
        .unwrap();
    for _ in 0..300 {
        super::native_core::observe_text("burst", [1.0; 4]);
    }
    native_events::dispatch(&h).unwrap();
    h.execute("assert(dropped==44);assert(#calls==258)")
        .unwrap();
}

#[test]
fn native_input_claims_conflict_across_vms_and_release_on_vm_exit() {
    super::input::reset_input_state();
    let a = Host::new().unwrap();
    let b = Host::new().unwrap();
    a.execute("claim=assert(eqoa.input.claim('Enter',true));assert(eqoa.input.capture_text(true))")
        .unwrap();
    b.execute("local v,e=eqoa.input.claim('Enter');assert(v==nil and e.code=='busy')")
        .unwrap();
    drop(a);
    b.execute("claim=assert(eqoa.input.claim('Enter'));assert(eqoa.input.release(claim)==true);assert(eqoa.input.release(claim)==false)").unwrap();
}

#[test]
fn native_settings_reject_nonplain_data_and_wait_for_a_real_writer() {
    let h = Host::new().unwrap();
    h.execute("for _,v in ipairs({{[1]=true,[3]=true},{[1]=true,a=true},setmetatable({},{}),{fn=function()end}}) do local ok,e=eqoa.settings.save(v);assert(ok==nil and e.code=='invalid_argument') end;local ok,e=eqoa.settings.save({size=20});assert(ok==nil and e.code=='unavailable')").unwrap();
}

#[test]
fn native_legacy_chat_queue_is_owned_bounded_and_cancelled_on_disable() {
    use super::{chat, native_events};
    chat::take_outgoing_chat();
    let a = Host::new().unwrap();
    let b = Host::new().unwrap();
    chat::install_chat_adapter(&a).unwrap();
    chat::install_chat_adapter(&b).unwrap();
    a.snapshot.set(super::host::Snapshot {
        in_game: true,
        ..Default::default()
    });
    b.snapshot.set(super::host::Snapshot {
        in_game: true,
        ..Default::default()
    });
    a.lua
        .globals()
        .set(
            "chat",
            a.registered_modules()["frontiers_forge.chat"].clone(),
        )
        .unwrap();
    b.lua
        .globals()
        .set(
            "chat",
            b.registered_modules()["frontiers_forge.chat"].clone(),
        )
        .unwrap();
    a.execute("assert(chat.SendChatText('a'));assert(chat.IsSendPending())")
        .unwrap();
    b.execute("assert(not chat.IsSendPending());assert(chat.SendChatText('b'))")
        .unwrap();
    native_events::clear(&a);
    assert_eq!(chat::take_outgoing_chat(), vec!["b"]);
    b.execute("for i=1,32 do assert(chat.SendChatText('queued')) end;assert(not chat.SendChatText('overflow'));assert(chat.UninstallSendHook());assert(not chat.IsSendPending());assert(not chat.SendChatText('after uninstall'))").unwrap();
    assert!(chat::take_outgoing_chat().is_empty());
}
#[test]
fn native_iteration_shares_instruction_budget_across_entry_and_callbacks() {
    let h = Host::new().unwrap();
    h.execute("UiForge.RegisterCallback(UiForge.CallbackType.Save,function() for i=1,2500000 do end;return 1 end)").unwrap();
    let entry = h.compile("for i=1,2500000 do end").unwrap();
    h.frame_entry(&entry).unwrap();
    assert!(
        h.callback(2, mlua::Value::Nil).is_err(),
        "entry and Save must share one 4M instruction budget"
    );
    h.end_iteration();
    assert!(
        h.callback(2, mlua::Value::Nil).is_ok(),
        "new top-level boundary replenishes the budget"
    );
}
#[test]
fn native_subscriber_heap_exhaustion_is_terminal() {
    let h = Host::new().unwrap();
    h.execute("eqoa.events.on('chat.message',function() local x=string.rep('x',32*1024*1024) end)")
        .unwrap();
    super::native_core::observe_text("heap test", [1.0; 4]);
    assert!(super::native_events::dispatch(&h).is_err());
}
