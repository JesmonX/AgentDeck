use crate::{model::*, process, store::Store};
use anyhow::Result;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, process::Command};
fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_default()
}
fn number(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}
fn delta(current: u64, old: Option<u64>) -> Option<u64> {
    old.and_then(|old| current.checked_sub(old))
}

pub fn sample(store: &Store) -> Result<Value> {
    anyhow::ensure!(cfg!(target_os = "linux"), "资源采集器仅支持 Linux");
    let previous = store.get::<Value>("raw_sample")?.unwrap_or(Value::Null);
    let timestamp = now();
    let boot = read("/proc/sys/kernel/random/boot_id").trim().to_string();
    let uptime = read("/proc/uptime")
        .split_whitespace()
        .next()
        .and_then(number)
        .unwrap_or(0.);
    let dt = uptime - previous["uptime"].as_f64().unwrap_or(uptime);
    let comparable = previous["bootId"] == boot && dt > 0. && dt < 300.;
    let mut cpu_raw = serde_json::Map::new();
    let mut cores = vec![];
    let mut overall = Value::Null;
    for line in read("/proc/stat").lines().filter(|s| s.starts_with("cpu")) {
        let mut p = line.split_whitespace();
        let name = p.next().unwrap_or("");
        let vals: Vec<u64> = p.take(8).filter_map(|s| s.parse().ok()).collect();
        if vals.len() < 5 {
            continue;
        }
        let total: u64 = vals.iter().sum();
        let idle = vals[3] + vals[4];
        let old = &previous["cpu"][name];
        let d = if comparable {
            delta(total, old["total"].as_u64())
        } else {
            None
        };
        let pct = |current: u64, key: &str| -> Value {
            if let Some(d) = d.filter(|v| *v > 0) {
                delta(current, old[key].as_u64())
                    .map(|v| json!(v as f64 / d as f64 * 100.))
                    .unwrap_or(Value::Null)
            } else {
                Value::Null
            }
        };
        let usage = pct(total - idle, "busy");
        let row = json!({"id":name,"usage":usage,"user":pct(vals[0]+vals[1],"user"),"system":pct(vals[2],"system"),"ioWait":pct(vals[4],"ioWait")});
        cpu_raw.insert(name.into(),json!({"total":total,"busy":total-idle,"user":vals[0]+vals[1],"system":vals[2],"ioWait":vals[4]}));
        if name == "cpu" {
            overall = row
        } else {
            cores.push(row)
        }
    }
    let mut mem = BTreeMap::new();
    for line in read("/proc/meminfo").lines() {
        let p: Vec<_> = line.split_whitespace().collect();
        if p.len() >= 2 {
            mem.insert(
                p[0].trim_end_matches(':').to_string(),
                p[1].parse::<u64>().unwrap_or(0) * 1024,
            );
        }
    }
    let m = |k: &str| *mem.get(k).unwrap_or(&0);
    let total = m("MemTotal");
    let available = m("MemAvailable");
    let mut network = vec![];
    let mut net_raw = serde_json::Map::new();
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut traffic = store
        .get::<Value>(&format!("traffic:{date}"))?
        .unwrap_or(json!({}));
    for line in read("/proc/net/dev").lines().skip(2) {
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        let p: Vec<u64> = rest
            .split_whitespace()
            .filter_map(|x| x.parse().ok())
            .collect();
        if p.len() < 16 {
            continue;
        }
        let old = &previous["network"][name];
        let (rx, tx) = if comparable {
            (
                delta(p[0], old["rx"].as_u64()),
                delta(p[8], old["tx"].as_u64()),
            )
        } else {
            (None, None)
        };
        let same_date = previous["date"] == date;
        let old_traffic = &traffic[name];
        let day_rx = old_traffic["received"].as_u64().unwrap_or(0)
            + if same_date { rx.unwrap_or(0) } else { 0 };
        let day_tx =
            old_traffic["sent"].as_u64().unwrap_or(0) + if same_date { tx.unwrap_or(0) } else { 0 };
        let gap = old_traffic["partial"].as_bool().unwrap_or(true) || rx.is_none() || !same_date;
        traffic[name] = json!({"received":day_rx,"sent":day_tx,"partial":gap});
        network.push(json!({"id":name,"receivedBytes":p[0],"sentBytes":p[8],"rxRate":rx.map(|n|n as f64/dt),"txRate":tx.map(|n|n as f64/dt),"dailyReceived":day_rx,"dailySent":day_tx,"partial":gap,"defaultVisible":name!="lo"&&!name.starts_with("veth")&&!name.starts_with("docker")&&!name.starts_with("br-")}));
        net_raw.insert(name.into(), json!({"rx":p[0],"tx":p[8]}));
    }
    store.set(&format!("traffic:{date}"), &traffic)?;
    let mut disks = vec![];
    let mut disk_error = None;
    match process::run(
        Command::new("df").args([
            "-B1", "-P", "-x", "tmpfs", "-x", "devtmpfs", "-x", "squashfs",
        ]),
        b"",
        5,
    ) {
        Ok(v) => {
            for line in String::from_utf8_lossy(&v).lines().skip(1) {
                let p: Vec<_> = line.split_whitespace().collect();
                if p.len() >= 6 {
                    let total = number(p[1]);
                    let used = number(p[2]);
                    disks.push(json!({"id":p[5..].join(" "),"device":p[0],"total":total,"used":used,"available":number(p[3]),"usage":number(p[4].trim_end_matches('%'))}));
                }
            }
        }
        Err(e) => disk_error = Some(e.to_string()),
    }
    if let Ok(v) = process::run(
        Command::new("df").args([
            "-i", "-P", "-x", "tmpfs", "-x", "devtmpfs", "-x", "squashfs",
        ]),
        b"",
        5,
    ) {
        for line in String::from_utf8_lossy(&v).lines().skip(1) {
            let p: Vec<_> = line.split_whitespace().collect();
            if p.len() >= 6
                && let Some(d) = disks.iter_mut().find(|d| d["id"] == p[5..].join(" "))
            {
                d["inodeUsage"] = json!(number(p[4].trim_end_matches('%')));
            }
        }
    }
    let mut disk_raw = serde_json::Map::new();
    let mut io = vec![];
    for line in read("/proc/diskstats").lines() {
        let p: Vec<_> = line.split_whitespace().collect();
        if p.len() < 14 || p[2].starts_with("loop") || p[2].starts_with("ram") {
            continue;
        }
        let get = |i: usize| p[i].parse::<u64>().unwrap_or(0);
        let name = p[2];
        let old = &previous["disks"][name];
        let rate = |n: u64, key: &str| {
            if comparable {
                delta(n, old[key].as_u64()).map(|n| n as f64 / dt)
            } else {
                None
            }
        };
        io.push(json!({"id":name,"readRate":rate(get(5)*512,"read"),"writeRate":rate(get(9)*512,"write"),"readIops":rate(get(3),"ri"),"writeIops":rate(get(7),"wi")}));
        disk_raw.insert(
            name.into(),
            json!({"read":get(5)*512,"write":get(9)*512,"ri":get(3),"wi":get(7)}),
        );
    }
    let mut gpus = vec![];
    let mut gpu_error = None;
    let mut processes = vec![];
    match process::run(Command::new("nvidia-smi").args(["--query-gpu=uuid,name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw","--format=csv,noheader,nounits"]),b"",5){Ok(v)=>{for line in String::from_utf8_lossy(&v).lines(){let p:Vec<_>=line.split(',').map(str::trim).collect();if p.len()>=7{gpus.push(json!({"id":p[0],"name":p[1],"usage":number(p[2]),"memoryUsed":number(p[3]).map(|n|n*1048576.),"memoryTotal":number(p[4]).map(|n|n*1048576.),"temperature":number(p[5]),"power":number(p[6])}));}}},Err(_)=>gpu_error=Some("未检测到可访问的 NVIDIA 驱动或 GPU".to_string())}
    if !gpus.is_empty()
        && let Ok(v) = process::run(
            Command::new("nvidia-smi").args([
                "--query-compute-apps=gpu_uuid,pid,process_name,used_memory",
                "--format=csv,noheader,nounits",
            ]),
            b"",
            5,
        )
    {
        for line in String::from_utf8_lossy(&v).lines() {
            let p: Vec<_> = line.split(',').map(str::trim).collect();
            if p.len() >= 4 {
                processes.push(json!({"gpu":p[0],"pid":p[1],"name":p[2],"memory":number(p[3]).map(|n|n*1048576.)}));
            }
        }
    }
    let load: Vec<f64> = read("/proc/loadavg")
        .split_whitespace()
        .take(3)
        .filter_map(number)
        .collect();
    let payload = json!({"timestamp":timestamp,"trafficDate":date,"hostname":read("/etc/hostname").trim(),"bootId":boot,"uptime":uptime,"cpu":overall,"cores":cores,"load":load,"memory":{"total":total,"used":total.saturating_sub(available),"available":available,"cached":m("Cached"),"swapTotal":m("SwapTotal"),"swapUsed":m("SwapTotal").saturating_sub(m("SwapFree"))},"network":network,"disks":disks,"diskIo":io,"gpus":gpus,"gpuProcesses":processes,"gpuError":gpu_error,"diskError":disk_error,"sampleCount":1});
    store.set("raw_sample",&json!({"bootId":boot,"uptime":uptime,"date":date,"cpu":cpu_raw,"network":net_raw,"disks":disk_raw}))?;
    let device = store.device()?;
    store.sample(&device, timestamp, 5, &payload, true)?;
    store.set("latest_sample", &payload)?;
    Ok(payload)
}

fn mean_values(rows: &[Value]) -> Value {
    let Some(last) = rows.last() else {
        return Value::Null;
    };
    if last.is_object() {
        let mut out = last.clone();
        for (k, _) in last.as_object().unwrap() {
            let vs: Vec<_> = rows.iter().filter_map(|v| v.get(k).cloned()).collect();
            if [
                "usage",
                "user",
                "system",
                "ioWait",
                "rxRate",
                "txRate",
                "readRate",
                "writeRate",
                "readIops",
                "writeIops",
                "power",
                "temperature",
                "used",
                "available",
                "memoryUsed",
            ]
            .contains(&k.as_str())
            {
                let ns: Vec<_> = vs.iter().filter_map(Value::as_f64).collect();
                out[k] = if ns.is_empty() {
                    Value::Null
                } else {
                    json!(ns.iter().sum::<f64>() / ns.len() as f64)
                }
            } else if vs.last().is_some_and(|v| v.is_object() || v.is_array()) {
                out[k] = mean_values(&vs);
            }
        }
        out
    } else if let Some(arr) = last.as_array() {
        json!(
            arr.iter()
                .enumerate()
                .map(|(i, item)| {
                    let matched: Vec<_> = rows
                        .iter()
                        .filter_map(|r| r.as_array())
                        .filter_map(|arr| {
                            if item.get("id").is_some() {
                                arr.iter().find(|x| x["id"] == item["id"]).cloned()
                            } else {
                                arr.get(i).cloned()
                            }
                        })
                        .collect();
                    mean_values(&matched)
                })
                .collect::<Vec<_>>()
        )
    } else {
        last.clone()
    }
}
pub fn compact(store: &Store) -> Result<()> {
    let device = store.device()?;
    let settings = store.settings()?;
    for (from, to) in [(5, 60), (60, 3600)] {
        let c = store.db()?;
        let cutoff = now() / to * to;
        let start = store
            .get::<i64>(&format!("rollup:{to}"))?
            .unwrap_or(cutoff - 2 * to);
        let mut st=c.prepare("SELECT timestamp,payload FROM samples WHERE device=? AND resolution=? AND timestamp>=? AND timestamp<? ORDER BY timestamp")?;
        let mut groups: BTreeMap<i64, Vec<Value>> = BTreeMap::new();
        for row in st.query_map(rusqlite::params![device, from, start, cutoff], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })? {
            let (t, p) = row?;
            groups
                .entry(t / to * to)
                .or_default()
                .push(serde_json::from_str(&p)?);
        }
        for (bucket, rows) in groups {
            let mut p = mean_values(&rows);
            p["timestamp"] = json!(bucket);
            p["sampleCount"] = json!(
                rows.iter()
                    .map(|r| r["sampleCount"].as_i64().unwrap_or(1))
                    .sum::<i64>()
            );
            store.sample(&device, bucket, to, &p, true)?;
        }
        store.set(&format!("rollup:{to}"), &cutoff)?;
    }
    let c = store.db()?;
    for (res, age) in [
        (5, settings.raw_hours * 3600),
        (60, settings.minute_days * 86400),
        (3600, settings.hour_days * 86400),
    ] {
        c.execute(
            "DELETE FROM samples WHERE resolution=? AND timestamp<?",
            rusqlite::params![res, now() - age],
        )?;
    }
    let pruned: i64 = c.query_row(
        "SELECT COALESCE(MAX(seq),0) FROM changes WHERE created<?",
        [now() - settings.raw_hours * 3600],
        |r| r.get(0),
    )?;
    if pruned > 0 {
        store.set("journal_pruned_through", &pruned)?;
    }
    c.execute(
        "DELETE FROM changes WHERE created<?",
        [now() - settings.raw_hours * 3600],
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reset_does_not_spike() {
        assert_eq!(delta(10, Some(20)), None);
        assert_eq!(delta(30, Some(20)), Some(10));
    }
    #[test]
    fn aggregate_preserves_last_counters() {
        let p = mean_values(&[
            json!({"usage":20,"receivedBytes":100}),
            json!({"usage":40,"receivedBytes":300}),
        ]);
        assert_eq!(p["usage"], 30.);
        assert_eq!(p["receivedBytes"], 300);
    }
}
