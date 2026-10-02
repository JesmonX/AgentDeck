pub mod model;
pub mod monitor;
pub mod pricing;
pub mod process;
pub mod quota;
pub mod ssh;
pub mod store;
pub mod usage;
use anyhow::{Context, Result, bail};
use model::*;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use store::Store;
pub(crate) static CONFIG_WRITE: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn read_ssh_secret(account: &str) -> Result<String> {
    anyhow::ensure!(
        account.starts_with("ssh-") && valid_id(account),
        "invalid account"
    );
    Ok(keyring::Entry::new("AgentDeck", account)?.get_password()?)
}
pub fn dispatch(store: &Store, method: &str, p: Value) -> Result<Value> {
    let id = || p["id"].as_str().context("缺少 ID");
    match method {
        "trafficHistory" => store.traffic(
            p["device"].as_str().context("缺少设备")?,
            p["since"].as_i64().unwrap_or(now() - 30 * 86400),
        ),
        "snapshot" => {
            let settings = store.settings()?;
            let mut servers = vec![];
            for s in &settings.servers {
                servers.push(
                    store
                        .get::<Value>(&format!("server:{}", s.id))?
                        .unwrap_or(json!({"id":s.id,"label":s.label,"status":"unconfigured"})),
                );
            }
            let mut sources = store
                .get::<Vec<Value>>("source_status")?
                .unwrap_or_default();
            for server in &servers {
                if let Some(remote_sources) = server
                    .pointer("/collector/sources")
                    .and_then(Value::as_array)
                {
                    for source in remote_sources {
                        let mut source = source.clone();
                        source["source"] = json!(format!(
                            "{} / {}",
                            server["label"].as_str().unwrap_or("远端"),
                            source["source"].as_str().unwrap_or("")
                        ));
                        sources.push(source);
                    }
                }
            }
            let prices = store.get::<Value>("prices")?.unwrap_or(Value::Null);
            Ok(
                json!({"settings":settings,"device":store.device()?,"usage":usage::aggregates(store)?,"sources":sources,"quotas":store.get::<Value>("quotas")?.unwrap_or(json!([])),"servers":servers,"priceUpdatedAt":prices["updatedAt"],"priceError":store.get::<Value>("price_error")?,"generatedAt":now()}),
            )
        }
        "settings" => Ok(serde_json::to_value(store.settings()?)?),
        "saveSettings" => {
            let _guard = CONFIG_WRITE
                .lock()
                .map_err(|_| anyhow::anyhow!("配置锁不可用"))?;
            let settings: Settings = serde_json::from_value(p)?;
            validate_settings(&settings)?;
            let old = store.settings()?;
            store.set("settings", &settings)?;
            let mut qs = store
                .get::<Vec<QuotaSnapshot>>("quotas")?
                .unwrap_or_default();
            qs.retain(|q| {
                settings.accounts.iter().any(|a| {
                    a.id == q.id
                        && old
                            .accounts
                            .iter()
                            .any(|b| serde_json::to_value(a).ok() == serde_json::to_value(b).ok())
                })
            });
            store.set("quotas", &qs)?;
            Ok(json!({"saved":true}))
        }
        "scan" => Ok(serde_json::to_value(usage::scan(store)?)?),
        "refreshQuota" => Ok(serde_json::to_value(quota::refresh(
            store,
            p["id"].as_str(),
        )?)?),
        "refreshPrices" => {
            let result = pricing::refresh(store);
            match &result {
                Ok(_) => store.set("price_error", &Value::Null)?,
                Err(e) => store.set("price_error", &e.to_string())?,
            }
            result
        }
        "saveSecret" => {
            quota::set_secret(id()?, p["value"].as_str().context("缺少密钥")?)?;
            Ok(json!({"saved":true}))
        }
        "serverProbe" => ssh::inspect(store, id()?),
        "serverKey" => ssh::host_key(store, id()?, p["accept"].as_bool().unwrap_or(false)),
        "serverInstall" => ssh::install(store, id()?, p["binaryPath"].as_str()),
        "serverUninstall" => {
            ssh::uninstall(store, id()?, p["deleteData"].as_bool().unwrap_or(false))
        }
        "serverSync" => ssh::sync(store, id()?),
        "history" => store.history(
            p["device"].as_str().context("缺少设备")?,
            p["since"].as_i64().unwrap_or(now() - 3600),
        ),
        _ => bail!("未知操作"),
    }
}
fn validate_settings(s: &Settings) -> Result<()> {
    anyhow::ensure!(
        (10..=3600).contains(&s.poll_seconds) && (60..=3600).contains(&s.quota_seconds),
        "刷新周期超出范围"
    );
    anyhow::ensure!((1..=16).contains(&s.concurrency), "并发范围为 1–16");
    anyhow::ensure!(
        (1..=720).contains(&s.raw_hours)
            && (1..=365).contains(&s.minute_days)
            && (30..=3650).contains(&s.hour_days),
        "数据保留范围无效"
    );
    let mut ids = std::collections::HashSet::new();
    for x in &s.servers {
        ssh::validate(x)?;
        anyhow::ensure!(ids.insert(&x.id), "服务器 ID 重复");
    }
    let mut ids = std::collections::HashSet::new();
    for x in &s.sources {
        anyhow::ensure!(valid_id(&x.id) && ids.insert(&x.id), "数据源 ID 无效或重复");
        anyhow::ensure!(
            ["codex", "claude", "agy"].contains(&x.agent.as_str()),
            "未知数据源类型"
        );
    }
    let mut ids = std::collections::HashSet::new();
    for a in &s.accounts {
        anyhow::ensure!(valid_id(&a.id) && ids.insert(&a.id), "账户 ID 无效或重复");
        anyhow::ensure!(
            ["codex", "claude", "agy", "deepseek"].contains(&a.provider.as_str()),
            "未知账户类型"
        );
        anyhow::ensure!(
            a.server_id.is_empty() || s.servers.iter().any(|x| x.id == a.server_id),
            "账户引用的服务器不存在"
        );
        anyhow::ensure!(
            a.provider != "deepseek" || a.server_id.is_empty(),
            "DeepSeek 余额由本机查询"
        );
    }
    for p in &s.price_overrides {
        for v in [p.input, p.output, p.cache_read, p.cache_write]
            .into_iter()
            .flatten()
        {
            anyhow::ensure!(v.is_finite() && v >= 0., "价格必须为非负数");
        }
    }
    Ok(())
}
pub fn rpc(store: &Store, method: &str, p: Value) -> Result<Value> {
    match method {
        "configure" => {
            let mut settings = store.settings()?;
            if let Some(v) = p["rawHours"].as_i64() {
                settings.raw_hours = v;
            }
            if let Some(v) = p["minuteDays"].as_i64() {
                settings.minute_days = v;
            }
            if let Some(v) = p["hourDays"].as_i64() {
                settings.hour_days = v;
            }
            validate_settings(&settings)?;
            store.set("settings", &settings)?;
            Ok(json!({"configured":true}))
        }
        "capabilities" => {
            let c = store.db()?;
            let watermark: i64 = c.query_row(
                "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='changes'),0)",
                [],
                |r| r.get(0),
            )?;
            Ok(
                json!({"version":1,"appVersion":env!("CARGO_PKG_VERSION"),"device":store.device()?,"watermark":watermark,"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"sources":store.get::<Value>("source_status")?.unwrap_or(json!([])),"lastSample":store.get::<Value>("latest_sample")?.map(|s|s["timestamp"].clone())}),
            )
        }
        "latest" => Ok(store.get::<Value>("latest_sample")?.unwrap_or(Value::Null)),
        "changes" => store.changes(p["after"].as_i64().unwrap_or(0)),
        "quota" => {
            let a = Account {
                id: "remote".into(),
                provider: p["provider"].as_str().context("provider")?.into(),
                executable: p["executable"].as_str().unwrap_or("").into(),
                credential_path: p["credentialPath"].as_str().unwrap_or("").into(),
                ..Default::default()
            };
            anyhow::ensure!(a.provider != "deepseek", "远端不托管 DeepSeek Key");
            Ok(serde_json::to_value(quota::fetch(&a)?)?)
        }
        "export" => {
            let c = store.db()?;
            let after = p["after"].as_str().unwrap_or("");
            let mut rows = vec![];
            let mut cursor = after.to_string();
            if p["kind"] == "usage" {
                let mut st =
                    c.prepare("SELECT id,event FROM usage WHERE id>? ORDER BY id LIMIT 500")?;
                for row in st.query_map([after], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })? {
                    let (id, payload) = row?;
                    cursor = id;
                    rows.push(serde_json::from_str::<Value>(&payload)?);
                }
            } else if p["kind"] == "samples" {
                let mut st=c.prepare("SELECT seq,device,timestamp,resolution,payload FROM samples WHERE seq>? ORDER BY seq LIMIT 500")?;
                for row in st.query_map([after.parse::<i64>().unwrap_or(0)], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, String>(4)?,
                    ))
                })? {
                    let (seq, device, timestamp, resolution, payload) = row?;
                    cursor = seq.to_string();
                    rows.push(json!({"device":device,"timestamp":timestamp,"resolution":resolution,"payload":serde_json::from_str::<Value>(&payload)?}));
                }
            } else {
                bail!("未知导出类型");
            }
            Ok(json!({"hasMore":rows.len()==500,"cursor":cursor,"rows":rows}))
        }
        _ => bail!("不支持的采集器操作"),
    }
}
pub fn start_desktop_workers(store: Store) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    for kind in ["usage", "quota", "server", "prices", "maintenance"] {
        let store = store.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let mut last = 0;
            while !stop.load(Ordering::Relaxed) {
                if let Ok(s) = store.settings() {
                    let interval = match kind {
                        "quota" => s.quota_seconds,
                        "prices" => 86400,
                        "maintenance" => 3600,
                        "server" => 15,
                        _ => s.poll_seconds,
                    } as i64;
                    if !s.paused && now() - last >= interval {
                        match kind {
                            "maintenance" => {
                                let _ = monitor::compact(&store);
                            }
                            "usage" => {
                                let _ = usage::scan(&store);
                            }
                            "quota" => {
                                let _ = quota::refresh(&store, None);
                            }
                            "prices" => {
                                let prior = store
                                    .get::<Value>("prices")
                                    .ok()
                                    .flatten()
                                    .and_then(|p| p["updatedAt"].as_i64())
                                    .unwrap_or(0);
                                if now() - prior >= 86400 {
                                    let _ = dispatch(&store, "refreshPrices", json!({}));
                                }
                            }
                            "server" => {
                                for chunk in s.servers.chunks(s.concurrency.max(1)) {
                                    std::thread::scope(|scope| {
                                        for server in chunk {
                                            let store = &store;
                                            scope.spawn(move || {
                                                let _ = ssh::sync(store, &server.id);
                                            });
                                        }
                                    });
                                }
                            }
                            _ => {}
                        }
                        last = now();
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        });
    }
    stop
}
