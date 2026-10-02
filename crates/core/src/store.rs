use crate::model::*;
use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Store {
    pub dir: PathBuf,
}
impl Store {
    pub fn traffic(&self, device: &str, since: i64) -> Result<Value> {
        let c = self.db()?;
        let mut st=c.prepare("SELECT timestamp,payload FROM samples WHERE device=? AND timestamp>=? ORDER BY timestamp,resolution DESC")?;
        let mut days: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();
        for row in st.query_map(params![device, since], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })? {
            let (t, p) = row?;
            let sample: Value = serde_json::from_str(&p)?;
            let day = sample["trafficDate"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| {
                    chrono::DateTime::from_timestamp(t, 0)
                        .unwrap_or_default()
                        .format("%Y-%m-%d")
                        .to_string()
                });
            if let Some(network) = sample["network"].as_array() {
                for n in network {
                    let Some(id) = n["id"].as_str() else { continue };
                    let key = format!("{day}|{id}");
                    days.insert(key,json!({"date":day,"id":id,"received":n["dailyReceived"],"sent":n["dailySent"],"partial":n["partial"],"defaultVisible":n["defaultVisible"]}));
                }
            }
        }
        Ok(json!(days.into_values().collect::<Vec<_>>()))
    }
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        std::fs::create_dir_all(dir.as_ref())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.as_ref(), std::fs::Permissions::from_mode(0o700))?;
        }
        let s = Self {
            dir: dir.as_ref().to_path_buf(),
        };
        s.db()?.execute_batch("PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY,value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS usage(id TEXT PRIMARY KEY, timestamp INTEGER NOT NULL, agent TEXT NOT NULL, model TEXT NOT NULL,device TEXT NOT NULL,event TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS usage_time ON usage(timestamp);
            CREATE TABLE IF NOT EXISTS cursors(path TEXT PRIMARY KEY,state TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS samples(seq INTEGER PRIMARY KEY AUTOINCREMENT,device TEXT NOT NULL,timestamp INTEGER NOT NULL,resolution INTEGER NOT NULL DEFAULT 5,payload TEXT NOT NULL,UNIQUE(device,timestamp,resolution));
            CREATE INDEX IF NOT EXISTS samples_query ON samples(device,resolution,timestamp);
            CREATE TABLE IF NOT EXISTS changes(seq INTEGER PRIMARY KEY AUTOINCREMENT,kind TEXT NOT NULL,id TEXT NOT NULL,payload TEXT NOT NULL,created INTEGER NOT NULL);
            PRAGMA user_version=1;")?;
        s.db()?.execute(
            "INSERT OR IGNORE INTO kv(key,value) VALUES('device_id',?)",
            [serde_json::to_string(&uuid::Uuid::new_v4().to_string())?],
        )?;
        Ok(s)
    }
    pub fn default_dir() -> PathBuf {
        std::env::var_os("AGENTDECK_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::data_local_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("agentdeck")
            })
    }
    pub fn db(&self) -> Result<Connection> {
        let c = Connection::open(self.dir.join("agentdeck.db"))?;
        c.busy_timeout(std::time::Duration::from_secs(10))?;
        Ok(c)
    }
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let c = self.db()?;
        let r = c.query_row("SELECT value FROM kv WHERE key=?", [key], |r| {
            r.get::<_, String>(0)
        });
        match r {
            Ok(v) => Ok(Some(serde_json::from_str(&v)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    pub fn set<T: serde::Serialize>(&self, key: &str, v: &T) -> Result<()> {
        self.db()?.execute(
            "INSERT INTO kv VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, serde_json::to_string(v)?],
        )?;
        Ok(())
    }
    pub fn settings(&self) -> Result<Settings> {
        Ok(self.get("settings")?.unwrap_or_default())
    }
    pub fn device(&self) -> Result<String> {
        self.get("device_id")?.context("missing device identity")
    }
    pub fn put_events(&self, events: &[UsageEvent], journal: bool) -> Result<()> {
        let mut c = self.db()?;
        let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for e in events {
            anyhow::ensure!(
                e.input >= 0
                    && e.output >= 0
                    && e.cache_read.unwrap_or(0) >= 0
                    && e.cache_write.unwrap_or(0) >= 0,
                "negative usage"
            );
            let payload = serde_json::to_string(e)?;
            // Global event identity makes replicated logs idempotent. The first device attribution is retained.
            let old: Option<String> = tx
                .query_row("SELECT event FROM usage WHERE id=?", [&e.id], |r| r.get(0))
                .ok();
            let mut owned = e.clone();
            if let Some(old) = old.as_ref() {
                let prior: UsageEvent = serde_json::from_str(old)?;
                // Replayed partial streaming records must not reduce completed usage.
                if prior.input >= owned.input
                    && prior.output >= owned.output
                    && (prior.input > owned.input || prior.output > owned.output)
                {
                    continue;
                }
                owned.device = prior.device;
                owned.source = prior.source;
            }
            let payload = if old.is_some() {
                serde_json::to_string(&owned)?
            } else {
                payload
            };
            if old.as_deref() == Some(&payload) {
                continue;
            }
            tx.execute("INSERT INTO usage VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET timestamp=excluded.timestamp,model=excluded.model,event=excluded.event",params![e.id,e.timestamp,e.agent,e.model,owned.device,payload])?;
            if journal {
                tx.execute(
                    "INSERT INTO changes(kind,id,payload,created) VALUES('usage',?,?,?)",
                    params![e.id, payload, now()],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn events(&self) -> Result<Vec<UsageEvent>> {
        let c = self.db()?;
        let mut st = c.prepare("SELECT event FROM usage ORDER BY timestamp")?;
        let mut out = vec![];
        for r in st.query_map([], |r| r.get::<_, String>(0))? {
            out.push(serde_json::from_str(&r?)?);
        }
        Ok(out)
    }
    pub fn sample(
        &self,
        device: &str,
        timestamp: i64,
        resolution: i64,
        payload: &Value,
        journal: bool,
    ) -> Result<()> {
        let mut c = self.db()?;
        let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let n = tx.execute(
            "INSERT OR IGNORE INTO samples(device,timestamp,resolution,payload) VALUES(?,?,?,?)",
            params![device, timestamp, resolution, payload.to_string()],
        )?;
        if journal && n > 0 {
            tx.execute("INSERT INTO changes(kind,id,payload,created) VALUES('sample',?,?,?)",params![format!("{device}:{timestamp}:{resolution}"),json!({"device":device,"timestamp":timestamp,"resolution":resolution,"payload":payload}).to_string(),now()])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn history(&self, device: &str, since: i64) -> Result<Value> {
        let c = self.db()?;
        let duration = now() - since;
        let settings = self.settings()?;
        let res = if duration <= (settings.raw_hours * 3600).min(2 * 3600) {
            5
        } else if duration <= (settings.minute_days * 86400).min(7 * 86400) {
            60
        } else {
            3600
        };
        let mut st=c.prepare("SELECT timestamp,payload FROM samples WHERE device=? AND timestamp>=? AND resolution=? ORDER BY timestamp")?;
        let mut rows = vec![];
        for row in st.query_map(params![device, since, res], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })? {
            let (t, p) = row?;
            let p: Value = serde_json::from_str(&p)?;
            // History charts do not need per-core/process inventories in every sample.
            rows.push(json!({"timestamp":t,"payload":{"cpu":p["cpu"],"memory":p["memory"],"network":p["network"]}}));
        }
        Ok(json!({"resolution":res,"rows":rows}))
    }
    pub fn changes(&self, after: i64) -> Result<Value> {
        let c = self.db()?;
        let mut st = c.prepare(
            "SELECT seq,kind,id,payload FROM changes WHERE seq>? ORDER BY seq LIMIT 500",
        )?;
        let mut rows = vec![];
        let mut cursor = after;
        for row in st.query_map([after], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })? {
            let (seq, kind, id, payload) = row?;
            cursor = seq;
            rows.push(json!({"seq":seq,"kind":kind,"id":id,"payload":serde_json::from_str::<Value>(&payload)?}));
        }
        let min: i64 = c.query_row("SELECT COALESCE(MIN(seq),0) FROM changes", [], |r| r.get(0))?;
        let max: i64 = c.query_row(
            "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name='changes'),0)",
            [],
            |r| r.get(0),
        )?;
        let pruned = self.get::<i64>("journal_pruned_through")?.unwrap_or(0);
        Ok(
            json!({"version":1,"device":self.device()?,"cursor":cursor,"gap":after>0&&(min>after+1||after<pruned||after>max),"hasMore":rows.len()==500,"rows":rows}),
        )
    }
}
