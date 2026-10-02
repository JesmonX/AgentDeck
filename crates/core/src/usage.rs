//! Provider-specific normalization. Never derive token counts from conversation text.
use crate::{model::*, store::Store};
use anyhow::{Context, Result, bail};
use rusqlite::{OpenFlags, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::Path,
};

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Cursor {
    offset: u64,
    session: String,
    model: String,
    input: i64,
    output: i64,
    read: i64,
    write: i64,
    signature: String,
    last_timestamp: i64,
    errors: usize,
    prefix: String,
}
fn int(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(Value::as_i64).filter(|x| *x >= 0)
}
fn stamp(v: &Value) -> Option<i64> {
    v.as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.timestamp())
        .or_else(|| {
            v.as_i64()
                .map(|n| if n > 10_000_000_000 { n / 1000 } else { n })
        })
}
fn id(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

fn parse_line(
    v: &Value,
    c: &mut Cursor,
    source: &Source,
    device: &str,
) -> Result<Option<UsageEvent>> {
    let t = v["type"].as_str().unwrap_or("");
    let p = &v["payload"];
    if source.agent == "codex" {
        if t == "session_meta" {
            if let Some(s) = p["id"].as_str() {
                c.session = s.into();
            }
            return Ok(None);
        }
        if t == "turn_context" {
            if let Some(s) = p["model"].as_str() {
                c.model = s.into();
            }
            return Ok(None);
        }
        if t != "event_msg" || p["type"] != "token_count" {
            return Ok(None);
        }
        let total = &p["info"]["total_token_usage"];
        if total.is_null() {
            return Ok(None);
        }
        let input = int(total, "input_tokens").context("invalid input count")?;
        let output = int(total, "output_tokens").context("invalid output count")?;
        let read = int(total, "cached_input_tokens");
        let write = int(total, "cache_write_input_tokens");
        let signature = total.to_string();
        if signature == c.signature {
            return Ok(None);
        }
        let reset = input < c.input || output < c.output;
        let di = if reset { input } else { input - c.input };
        let do_ = if reset { output } else { output - c.output };
        let dr = read.map(|r| {
            if reset {
                r
            } else {
                r.saturating_sub(c.read).max(0)
            }
        });
        let dw = write.map(|r| {
            if reset {
                r
            } else {
                r.saturating_sub(c.write).max(0)
            }
        });
        let ts = stamp(&v["timestamp"]).context("missing usage timestamp")?;
        let e = UsageEvent {
            id: id(&format!("codex:{}:{ts}:{signature}", c.session)),
            source: source.id.clone(),
            device: device.into(),
            agent: "codex".into(),
            session: c.session.clone(),
            timestamp: ts,
            model: if c.model.is_empty() {
                "unknown".into()
            } else {
                c.model.clone()
            },
            input: di,
            output: do_,
            cache_read: dr,
            cache_write: dw,
            reasoning: None,
        };
        c.input = input;
        c.output = output;
        c.read = read.unwrap_or(0);
        c.write = write.unwrap_or(0);
        c.signature = signature;
        c.last_timestamp = ts;
        return Ok((di + do_ > 0).then_some(e));
    }
    if source.agent == "claude" {
        if t != "assistant" {
            return Ok(None);
        }
        let m = &v["message"];
        let u = &m["usage"];
        if u.is_null() {
            return Ok(None);
        }
        let raw_input = int(u, "input_tokens").context("invalid Claude input")?;
        let output = int(u, "output_tokens").context("invalid Claude output")?;
        if m.get("stop_reason") == Some(&Value::Null) && output == 0 {
            return Ok(None);
        }
        let read = int(u, "cache_read_input_tokens");
        let write = int(u, "cache_creation_input_tokens");
        let input = raw_input
            .checked_add(read.unwrap_or(0))
            .and_then(|x| x.checked_add(write.unwrap_or(0)))
            .context("token overflow")?;
        let session = v["sessionId"]
            .as_str()
            .or(v["session_id"].as_str())
            .unwrap_or(&c.session);
        let response = m["id"]
            .as_str()
            .or(v["uuid"].as_str())
            .context("missing response identity")?;
        return Ok(Some(UsageEvent {
            id: id(&format!("claude:{session}:{response}")),
            source: source.id.clone(),
            device: device.into(),
            agent: "claude".into(),
            session: session.into(),
            timestamp: stamp(&v["timestamp"]).context("missing timestamp")?,
            model: m["model"].as_str().unwrap_or("unknown").into(),
            input,
            output,
            cache_read: read,
            cache_write: write,
            reasoning: int(&u["output_tokens_details"], "thinking_tokens"),
        }));
    }
    bail!("unsupported source")
}

pub fn scan(store: &Store) -> Result<Vec<SourceStatus>> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock().map_err(|_| anyhow::anyhow!("扫描锁不可用"))?;
    let settings = store.settings()?;
    let device = store.device()?;
    let mut statuses = vec![];
    for source in settings.sources.iter().filter(|s| s.enabled) {
        let root = Path::new(&source.path);
        let mut count = 0;
        let mut errors = 0;
        let mut files = 0;
        let started = std::time::Instant::now();
        let mut pending = false;
        if !root.exists() {
            statuses.push(SourceStatus {
                source: source.id.clone(),
                status: "unavailable".into(),
                scanned_at: now(),
                events: 0,
                errors: 0,
                message: "数据目录不存在".into(),
            });
            continue;
        }
        for entry in walkdir::WalkDir::new(root)
            .follow_links(false)
            .max_depth(10)
        {
            if started.elapsed().as_secs() >= 12 {
                pending = true;
                break;
            }
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    errors += 1;
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if source.agent == "agy" {
                if ext != "db"
                    || path.file_stem().and_then(|s| s.to_str()) == Some("conversation_summaries")
                {
                    continue;
                }
                files += 1;
                let fingerprint = |p: &Path| -> String {
                    std::fs::metadata(p)
                        .map(|m| format!("{}:{:?}", m.len(), m.modified().ok()))
                        .unwrap_or_default()
                };
                let signature = format!(
                    "{}:{}",
                    fingerprint(path),
                    fingerprint(&std::path::PathBuf::from(format!("{}-wal", path.display())))
                );
                let key = format!("agy-file:{}:{}", source.id, path.display());
                if let Some(cached) = store.get::<Value>(&key)?
                    && cached["signature"] == signature
                {
                    errors += cached["errors"].as_u64().unwrap_or(0) as usize;
                    continue;
                }
                match scan_agy(path, source, &device) {
                    Ok((events, skipped)) => {
                        count += events.len();
                        errors += skipped;
                        store.put_events(&events, true)?;
                        store.set(&key, &json!({"signature":signature,"errors":skipped}))?;
                    }
                    Err(_) => errors += 1,
                }
                continue;
            }
            if ext != "jsonl" {
                continue;
            }
            files += 1;
            let key = format!("{}:{}", source.id, path.display());
            let db = store.db()?;
            let saved: Option<String> = db
                .query_row("SELECT state FROM cursors WHERE path=?", [&key], |r| {
                    r.get(0)
                })
                .ok();
            let mut cursor: Cursor = saved
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            let mut file = match File::open(path) {
                Ok(f) => f,
                Err(_) => {
                    errors += 1;
                    continue;
                }
            };
            use std::io::Read;
            let mut prefix = [0u8; 256];
            let n = file.read(&mut prefix)?;
            let fingerprint = hex::encode(Sha256::digest(&prefix[..n]));
            if !cursor.prefix.is_empty() && cursor.prefix != fingerprint {
                cursor = Cursor::default();
            }
            if file.metadata()?.len() < cursor.offset {
                cursor = Cursor::default();
            }
            cursor.prefix = fingerprint;
            if cursor.session.is_empty() {
                cursor.session = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into();
            }
            file.seek(SeekFrom::Start(cursor.offset))?;
            let mut reader = BufReader::new(file);
            let mut events = vec![];
            loop {
                let mut line = String::new();
                let n = reader.read_line(&mut line)?;
                if n == 0 {
                    break;
                }
                if !line.ends_with('\n') {
                    break;
                } // don't commit a partially written JSON record
                cursor.offset += n as u64;
                if line.len() > 16 * 1024 * 1024 {
                    cursor.errors += 1;
                    continue;
                }
                match serde_json::from_str::<Value>(&line)
                    .map_err(anyhow::Error::from)
                    .and_then(|v| parse_line(&v, &mut cursor, source, &device))
                {
                    Ok(Some(e)) => events.push(e),
                    Ok(None) => {}
                    Err(_) => cursor.errors += 1,
                }
                if events.len() >= 500 {
                    count += events.len();
                    store.put_events(&events, true)?;
                    events.clear();
                    if started.elapsed().as_secs() >= 12 {
                        pending = true;
                        break;
                    }
                }
            }
            count += events.len();
            errors += cursor.errors;
            store.put_events(&events, true)?;
            db.execute("INSERT INTO cursors VALUES(?,?) ON CONFLICT(path) DO UPDATE SET state=excluded.state",params![key,serde_json::to_string(&cursor)?])?;
        }
        statuses.push(SourceStatus {
            source: source.id.clone(),
            status: if files == 0 {
                "unavailable"
            } else if errors > 0 || pending {
                "partial"
            } else {
                "ready"
            }
            .into(),
            scanned_at: now(),
            events: count,
            errors,
            message: format!(
                "读取 {files} 个文件；本轮 {count} 条事件；{errors} 项未解析{}",
                if pending {
                    "；历史正在分批索引"
                } else {
                    ""
                }
            ),
        });
    }
    store.set("source_status", &statuses)?;
    Ok(statuses)
}

// Small bounded protobuf wire reader; ignores unknown fields without interpreting text.
#[derive(Clone, Debug)]
enum Field {
    Number(u64),
    Bytes(Vec<u8>),
}
type Fields = HashMap<u32, Vec<Field>>;
fn varint(b: &[u8], i: &mut usize) -> Result<u64> {
    let mut v = 0;
    for shift in (0..70).step_by(7) {
        let x = *b.get(*i).context("truncated varint")?;
        *i += 1;
        if shift == 63 && x > 1 {
            bail!("overflow")
        };
        v |= ((x & 127) as u64) << shift;
        if x < 128 {
            return Ok(v);
        }
    }
    bail!("invalid varint")
}
fn decode(b: &[u8]) -> Result<Fields> {
    anyhow::ensure!(b.len() <= 16 * 1024 * 1024, "oversize protobuf");
    let (mut i, mut f) = (0, HashMap::new());
    while i < b.len() {
        let k = varint(b, &mut i)?;
        let tag = (k >> 3) as u32;
        anyhow::ensure!(tag > 0, "invalid tag");
        let value = match k & 7 {
            0 => Field::Number(varint(b, &mut i)?),
            2 => {
                let len = usize::try_from(varint(b, &mut i)?)?;
                let end = i.checked_add(len).context("overflow")?;
                let v = b.get(i..end).context("truncated bytes")?.to_vec();
                i = end;
                Field::Bytes(v)
            }
            1 => {
                i += 8;
                continue;
            }
            5 => {
                i += 4;
                continue;
            }
            _ => bail!("unsupported wire type"),
        };
        f.entry(tag).or_insert_with(Vec::new).push(value);
    }
    anyhow::ensure!(i == b.len(), "truncated field");
    Ok(f)
}
fn bytes(f: &Fields, n: u32) -> Option<&[u8]> {
    match f.get(&n)?.first()? {
        Field::Bytes(b) => Some(b),
        _ => None,
    }
}
fn num(f: &Fields, n: u32) -> Option<i64> {
    match f.get(&n)?.first()? {
        Field::Number(v) => i64::try_from(*v).ok(),
        _ => None,
    }
}
fn sub(f: &Fields, n: u32) -> Fields {
    bytes(f, n).and_then(|b| decode(b).ok()).unwrap_or_default()
}
fn text(f: &Fields, n: u32) -> Option<String> {
    String::from_utf8(bytes(f, n)?.to_vec()).ok()
}
fn proto_time(f: &Fields, n: u32) -> Option<i64> {
    num(&sub(f, n), 1).filter(|x| *x > 1_500_000_000 && *x < 4_000_000_000)
}
fn scan_agy(path: &Path, source: &Source, device: &str) -> Result<(Vec<UsageEvent>, usize)> {
    let db = rusqlite::Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.busy_timeout(std::time::Duration::from_secs(2))?;
    let has: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='gen_metadata' AND type='table')",
        [],
        |r| r.get(0),
    )?;
    if !has {
        return Ok((vec![], 0));
    }
    let mut times: HashMap<(String, String), i64> = HashMap::new();
    let mut st = db.prepare("SELECT metadata FROM steps")?;
    for row in st.query_map([], |r| r.get::<_, Vec<u8>>(0))? {
        let Ok(b) = row else { continue };
        let Ok(f) = decode(&b) else { continue };
        if let (Some(step), Some(t)) = (text(&f, 12), proto_time(&f, 1)) {
            let bot = text(&sub(&f, 9), 7).unwrap_or_default();
            let k = (step, bot);
            if let Some(old) = times.get_mut(&k) {
                if *old != t {
                    *old = 0;
                }
            } else {
                times.insert(k, t);
            }
        }
    }
    let session = path.file_stem().unwrap_or_default().to_string_lossy();
    let mut out = vec![];
    let mut skipped = 0;
    let mut st = db.prepare("SELECT idx,data FROM gen_metadata ORDER BY idx")?;
    for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?)))? {
        let (idx, b) = row?;
        let parsed = (|| -> Result<Option<UsageEvent>> {
            let f = decode(&b)?;
            let chat = sub(&f, 1);
            if chat.is_empty() {
                return Ok(None);
            }
            let u = sub(&chat, 4);
            if u.is_empty() {
                return Ok(None);
            }
            let timestamp = proto_time(&sub(&chat, 9), 4)
                .or_else(|| {
                    let step = text(&f, 4)?;
                    let bot = text(&u, 7).unwrap_or_default();
                    times
                        .get(&(step.clone(), bot))
                        .or_else(|| times.get(&(step, String::new())))
                        .copied()
                        .filter(|t| *t > 0)
                })
                .context("unrecognized timestamp")?;
            let input = num(&u, 2).context("missing input")?;
            let output = match (num(&u, 9), num(&u, 10)) {
                (Some(a), Some(b)) => a.checked_add(b).context("overflow")?,
                _ => num(&u, 3).context("missing output")?,
            };
            let read = num(&u, 5).unwrap_or(0);
            // agy usage.input_tokens is uncached input; cache reads are a separate bucket.
            let input = input.checked_add(read).context("overflow")?;
            Ok(Some(UsageEvent {
                id: id(&format!("agy:{session}:{idx}")),
                source: source.id.clone(),
                device: device.into(),
                agent: "agy".into(),
                session: session.to_string(),
                timestamp,
                model: text(&chat, 19)
                    .unwrap_or_else(|| format!("agy-model-{}", num(&u, 1).unwrap_or(0))),
                input,
                output,
                cache_read: Some(read),
                cache_write: None,
                reasoning: num(&u, 9),
            }))
        })();
        match parsed {
            Ok(Some(e)) => out.push(e),
            Ok(None) => {}
            Err(_) => skipped += 1,
        }
    }
    Ok((out, skipped))
}

pub fn aggregates(store: &Store) -> Result<Value> {
    let events = store.events()?;
    let estimator = crate::pricing::Estimator::load(store)?;
    let mut buckets: HashMap<String, Value> = HashMap::new();
    // Minute buckets preserve local day boundaries, including fractional-hour zones.
    for e in events {
        let minute = e.timestamp / 60 * 60;
        let k = format!("{}|{}|{}|{}", minute, e.agent, e.model, e.device);
        let b=buckets.entry(k).or_insert_with(||json!({"timestamp":minute,"agent":e.agent,"model":e.model,"device":e.device,"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"cacheKnownInput":0,"requests":0,"cost":0.0,"pricedTokens":0}));
        for (key, n) in [
            ("input", e.input),
            ("output", e.output),
            ("cacheRead", e.cache_read.unwrap_or(0)),
            ("cacheWrite", e.cache_write.unwrap_or(0)),
            (
                "cacheKnownInput",
                if e.cache_read.is_some() { e.input } else { 0 },
            ),
            ("requests", 1),
        ] {
            b[key] = json!(
                b[key]
                    .as_i64()
                    .unwrap_or(0)
                    .checked_add(n)
                    .context("aggregate overflow")?
            );
        }
        if let Some(cost) = estimator.estimate(&e) {
            b["cost"] = json!(b["cost"].as_f64().unwrap_or(0.) + cost);
            b["pricedTokens"] = json!(b["pricedTokens"].as_i64().unwrap_or(0) + e.input + e.output);
        }
    }
    let mut rows: Vec<_> = buckets.into_values().collect();
    rows.sort_by_key(|r| r["timestamp"].as_i64());
    Ok(json!(rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(agent: &str) -> Source {
        Source {
            id: "test".into(),
            agent: agent.into(),
            path: "".into(),
            enabled: true,
        }
    }
    #[test]
    fn claude_cache_and_dedup() {
        let v = json!({"type":"assistant","sessionId":"s","timestamp":"2026-01-01T00:00:00Z","message":{"id":"r","model":"m","usage":{"input_tokens":10,"output_tokens":4,"cache_read_input_tokens":20,"cache_creation_input_tokens":5}}});
        let e = parse_line(&v, &mut Cursor::default(), &source("claude"), "d")
            .unwrap()
            .unwrap();
        assert_eq!(e.input, 35);
        let tmp = tempfile::tempdir().unwrap();
        let s = Store::open(tmp.path()).unwrap();
        s.put_events(&[e.clone(), e], true).unwrap();
        assert_eq!(s.events().unwrap().len(), 1);
    }
    #[test]
    fn codex_delta_repeated_and_partial() {
        let mut c = Cursor::default();
        let make = |n| json!({"type":"event_msg","timestamp":"2026-01-01T00:00:00Z","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":n,"output_tokens":5,"cached_input_tokens":3}}}});
        assert_eq!(
            parse_line(&make(10), &mut c, &source("codex"), "d")
                .unwrap()
                .unwrap()
                .input,
            10
        );
        assert!(
            parse_line(&make(10), &mut c, &source("codex"), "d")
                .unwrap()
                .is_none()
        );
        assert_eq!(
            parse_line(&make(20), &mut c, &source("codex"), "d")
                .unwrap()
                .unwrap()
                .input,
            10
        );
    }
    #[test]
    fn protobuf_rejects_truncation() {
        assert!(decode(&[0x0a, 0xff]).is_err());
        assert!(decode(&[0x08, 0x96, 0x01]).is_ok());
    }
}
