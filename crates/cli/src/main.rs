use agentdeck_core::{model::now, store::Store};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("{e:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    if let Ok(account) = std::env::var("AGENTDECK_ASKPASS_ACCOUNT") {
        let prompt = std::env::args().nth(1).unwrap_or_default().to_lowercase();
        anyhow::ensure!(
            prompt.contains("password") || prompt.contains("passphrase"),
            "不支持的 SSH 提示"
        );
        let value = agentdeck_core::read_ssh_secret(&account)?;
        println!("{value}");
        return Ok(());
    }
    let args: Vec<_> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("help");
    if mode == "version" {
        println!(
            "AgentDeck collector {} protocol=1",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if mode == "help" {
        println!(
            "agentdeck-collector <daemon|rpc|scan|sample|snapshot|serve|version>\nAGENTDECK_DATA_DIR overrides storage. serve requires AGENTDECK_DEV_TOKEN."
        );
        return Ok(());
    }
    let store = Store::open(Store::default_dir())?;
    match mode {
        "command" => {
            let mut input = String::new();
            std::io::stdin()
                .take(1024 * 1024)
                .read_to_string(&mut input)?;
            let params = if input.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&input)?
            };
            let method = args.get(2).context("command requires an operation name")?;
            println!("{}", agentdeck_core::dispatch(&store, method, params)?);
        }
        "daemon" => {
            let s = store.clone();
            std::thread::spawn(move || {
                loop {
                    let _ = agentdeck_core::usage::scan(&s);
                    std::thread::sleep(std::time::Duration::from_secs(60));
                }
            });
            let mut last = 0;
            loop {
                let started = std::time::Instant::now();
                if let Err(e) = agentdeck_core::monitor::sample(&store) {
                    eprintln!("sample: {e}");
                }
                if now() - last >= 60 {
                    if let Err(e) = agentdeck_core::monitor::compact(&store) {
                        eprintln!("compact: {e}");
                    }
                    last = now();
                }
                std::thread::sleep(
                    std::time::Duration::from_secs(5).saturating_sub(started.elapsed()),
                );
            }
        }
        "rpc" => {
            let mut buf = String::new();
            std::io::stdin()
                .take(1024 * 1024)
                .read_to_string(&mut buf)?;
            let request: Value = serde_json::from_str(&buf)?;
            anyhow::ensure!(request["version"] == 1, "protocol mismatch");
            let result = agentdeck_core::rpc(
                &store,
                request["method"].as_str().context("method")?,
                request["params"].clone(),
            );
            println!(
                "{}",
                match result {
                    Ok(v) => json!({"version":1,"result":v}),
                    Err(e) => json!({"version":1,"error":e.to_string()}),
                }
            );
        }
        "scan" => println!(
            "{}",
            serde_json::to_string(&agentdeck_core::usage::scan(&store)?)?
        ),
        "sample" => println!("{}", agentdeck_core::monitor::sample(&store)?),
        "snapshot" => println!(
            "{}",
            agentdeck_core::dispatch(&store, "snapshot", json!({}))?
        ),
        "serve" => serve(store)?,
        _ => anyhow::bail!("unknown command"),
    }
    Ok(())
}
fn serve(store: Store) -> Result<()> {
    let token =
        std::env::var("AGENTDECK_DEV_TOKEN").context("serve requires a development token")?;
    anyhow::ensure!(token.len() >= 32, "development token too short");
    let listener = TcpListener::bind("127.0.0.1:47831")?;
    let _workers = agentdeck_core::start_desktop_workers(store.clone());
    eprintln!("AgentDeck local API: 127.0.0.1:47831");
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let store = store.clone();
        let token = token.clone();
        std::thread::spawn(move || {
            let handle = (|| -> Result<Value> {
                stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
                stream.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;
                let mut data = vec![];
                let mut b = [0u8; 4096];
                let end = loop {
                    let n = stream.read(&mut b)?;
                    anyhow::ensure!(n > 0, "empty request");
                    data.extend_from_slice(&b[..n]);
                    anyhow::ensure!(data.len() < 2 * 1024 * 1024, "request too large");
                    if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let header = std::str::from_utf8(&data[..end])?;
                anyhow::ensure!(header.starts_with("POST /api "), "unsupported route");
                let headers: Vec<_> = header
                    .lines()
                    .filter_map(|l| l.split_once(':'))
                    .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string()))
                    .collect();
                anyhow::ensure!(
                    headers
                        .iter()
                        .any(|(k, v)| k == "authorization" && v == &format!("Bearer {token}")),
                    "unauthorized"
                );
                anyhow::ensure!(
                    headers.iter().any(|(k, v)| k == "content-type"
                        && v.split(';').next() == Some("application/json")),
                    "JSON content type required"
                );
                if let Some((_, origin)) = headers.iter().find(|(k, _)| k == "origin") {
                    anyhow::ensure!(
                        ["http://127.0.0.1:1420", "http://localhost:1420"]
                            .contains(&origin.as_str()),
                        "untrusted origin"
                    );
                }
                let len = headers
                    .iter()
                    .find(|(k, _)| k == "content-length")
                    .context("content length required")?
                    .1
                    .parse::<usize>()?;
                anyhow::ensure!(len <= 1024 * 1024, "request too large");
                while data.len() < end + len {
                    let n = stream.read(&mut b)?;
                    anyhow::ensure!(n > 0, "truncated body");
                    data.extend_from_slice(&b[..n]);
                }
                let request: Value = serde_json::from_slice(&data[end..end + len])?;
                agentdeck_core::dispatch(
                    &store,
                    request["method"].as_str().context("method")?,
                    request["params"].clone(),
                )
            })();
            let (status, payload) = match handle {
                Ok(v) => ("200 OK", json!({"result":v})),
                Err(e) => ("400 Bad Request", json!({"error":e.to_string()})),
            };
            let body = payload.to_string();
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
                body.len()
            );
        });
    }
    Ok(())
}
