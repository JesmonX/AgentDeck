use agentdeck_core::{model::*, monitor, rpc, store::Store, usage};
use serde_json::{Value, json};
use std::io::Write;

#[test]
fn account_changes_invalidate_cached_identity_but_theme_changes_do_not() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let mut settings = store.settings().unwrap();
    let id = settings.accounts[0].id.clone();
    store
        .set(
            "quotas",
            &vec![QuotaSnapshot {
                id: id.clone(),
                account_id: Some("old-identity".into()),
                ..Default::default()
            }],
        )
        .unwrap();
    settings.theme = "light".into();
    agentdeck_core::dispatch(
        &store,
        "saveSettings",
        serde_json::to_value(&settings).unwrap(),
    )
    .unwrap();
    assert_eq!(
        store
            .get::<Vec<QuotaSnapshot>>("quotas")
            .unwrap()
            .unwrap()
            .len(),
        1
    );
    settings.accounts[0].executable = "/different/codex".into();
    agentdeck_core::dispatch(
        &store,
        "saveSettings",
        serde_json::to_value(&settings).unwrap(),
    )
    .unwrap();
    assert!(
        store
            .get::<Vec<QuotaSnapshot>>("quotas")
            .unwrap()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn resume_partial_record_and_replay_without_double_counting() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");
    std::fs::create_dir(&logs).unwrap();
    let path = logs.join("session.jsonl");
    let store = Store::open(dir.path().join("state")).unwrap();
    let mut config = store.settings().unwrap();
    config.sources = vec![Source {
        id: "local".into(),
        agent: "codex".into(),
        path: logs.to_string_lossy().into(),
        enabled: true,
    }];
    store.set("settings", &config).unwrap();
    let event = |input| {
        json!({"type":"event_msg","timestamp":"2026-10-01T12:00:00Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":input,"output_tokens":10,"cached_input_tokens":20}}}}).to_string()
    };
    let mut file = std::fs::File::create(&path).unwrap();
    writeln!(
        file,
        "{}",
        json!({"type":"session_meta","payload":{"id":"session-one"}})
    )
    .unwrap();
    writeln!(
        file,
        "{}",
        json!({"type":"turn_context","payload":{"model":"gpt-test"}})
    )
    .unwrap();
    writeln!(file, "{}", event(100)).unwrap();
    write!(file, "{}", &event(150)[..40]).unwrap();
    drop(file);
    usage::scan(&store).unwrap();
    assert_eq!(store.events().unwrap().len(), 1);
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(file, "{}", &event(150)[40..]).unwrap();
    drop(file);
    usage::scan(&store).unwrap();
    usage::scan(&store).unwrap();
    let events = store.events().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events.iter().map(|e| e.input).sum::<i64>(), 150);
    assert_eq!(events.iter().map(|e| e.output).sum::<i64>(), 10);
}
#[test]
fn replicated_events_and_older_streaming_records_do_not_reduce_totals() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let a = UsageEvent {
        id: "response".into(),
        device: "first".into(),
        input: 100,
        output: 20,
        cache_read: Some(30),
        ..Default::default()
    };
    store.put_events(std::slice::from_ref(&a), true).unwrap();
    let mut b = a;
    b.device = "second".into();
    b.output = 5;
    store.put_events(&[b], true).unwrap();
    let e = store.events().unwrap();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].output, 20);
    assert_eq!(e[0].device, "first");
}
#[test]
fn rollup_and_retention_keep_history_and_signal_journal_gaps() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    let dev = store.device().unwrap();
    let t = now() / 3600 * 3600 - 3600;
    for i in 0..12 {
        store
            .sample(
                &dev,
                t + i * 5,
                5,
                &json!({"timestamp":t+i*5,"cpu":{"usage":i*2},"network":[],"sampleCount":1}),
                true,
            )
            .unwrap();
    }
    store.set("rollup:60", &t).unwrap();
    store.set("rollup:3600", &t).unwrap();
    monitor::compact(&store).unwrap();
    let c = store.db().unwrap();
    let p: String = c
        .query_row(
            "SELECT payload FROM samples WHERE resolution=3600",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let p: Value = serde_json::from_str(&p).unwrap();
    assert_eq!(p["cpu"]["usage"], 11.0);
    assert_eq!(p["sampleCount"], 12);
    c.execute("UPDATE changes SET created=?", [now() - 100 * 3600])
        .unwrap();
    monitor::compact(&store).unwrap();
    assert_eq!(store.changes(1).unwrap()["gap"], true);
    let cap = rpc(&store, "capabilities", json!({})).unwrap();
    assert!(cap["watermark"].as_i64().unwrap() > 0);
    let page = rpc(&store, "export", json!({"kind":"samples","after":""})).unwrap();
    assert!(page["rows"].as_array().unwrap().len() >= 2);
}
#[test]
fn traffic_uses_last_counter_per_interface_day_not_sum_of_samples() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    for (t, received) in [(100, 10), (110, 30)] {
        store.sample("s",t,5,&json!({"trafficDate":"2026-10-01","network":[{"id":"eth0","dailyReceived":received,"dailySent":5,"partial":false}]}),false).unwrap();
    }
    let v = store.traffic("s", 0).unwrap();
    assert_eq!(v[0]["received"], 30);
    assert_eq!(v[0]["sent"], 5);
}
#[test]
fn agy_group_windows_are_distinct_and_zero_remaining_is_known() {
    let v = json!({"status":"SUCCESS","command":{"name":"usage","data":{"groups":[{"name":"Gemini","buckets":[{"id":"w","window":"weekly","remaining_fraction":0.0,"reset_time":"2026-10-05T00:00:00Z"},{"id":"s","window":"5h","remaining_fraction":0.25,"reset_time":"2026-10-02T15:00:00Z"}]}]}}});
    let q = agentdeck_core::quota::parse_agy(&v).unwrap();
    assert_eq!(q.len(), 2);
    assert_eq!(q[0].used_percent, 100.);
    assert_eq!(q[1].duration_minutes, Some(300));
}
