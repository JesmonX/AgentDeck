use crate::{model::*, process, store::Store};
static RESOURCES: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
pub fn set_resources(path: PathBuf) {
    let _ = RESOURCES.set(path);
}
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

fn safe_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() < 256
        && !host.starts_with('-')
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@._-:[]".contains(&b))
}
pub fn validate(server: &Server) -> Result<()> {
    anyhow::ensure!(
        valid_id(&server.id),
        "设备 ID 仅支持字母、数字、下划线与短横线"
    );
    anyhow::ensure!(safe_host(&server.host), "SSH 地址格式无效");
    anyhow::ensure!(
        server.jump_host.is_empty() || safe_host(&server.jump_host),
        "跳板机地址格式无效"
    );
    anyhow::ensure!(server.port != Some(0), "SSH 端口无效");
    Ok(())
}
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
fn cmd(store: &Store, s: &Server) -> Result<Command> {
    validate(s)?;
    let mut c = Command::new("ssh");
    c.args([
        "-T",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "ServerAliveInterval=10",
        "-o",
        "ServerAliveCountMax=2",
        "-o",
        "StrictHostKeyChecking=yes",
    ]);
    let known = store.dir.join("known_hosts");
    let user = dirs::home_dir()
        .unwrap_or_default()
        .join(".ssh/known_hosts");
    c.args([
        "-o",
        &format!(
            "UserKnownHostsFile=\"{}\" \"{}\"",
            known.display(),
            user.display()
        ),
    ]);
    if let Some(p) = s.port {
        c.args(["-p", &p.to_string()]);
    }
    if !s.identity_file.is_empty() {
        c.arg("-i").arg(&s.identity_file);
    }
    if !s.jump_host.is_empty() {
        c.arg("-J").arg(&s.jump_host);
    }
    // Password/passphrase is fetched by our askpass helper from the OS vault, never placed in argv.
    let password = keyring::Entry::new("AgentDeck", &format!("ssh-{}", s.id))
        .ok()
        .and_then(|e| e.get_password().ok());
    if password.is_some() {
        let helper = collector_binary()?;
        c.env("SSH_ASKPASS", helper)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env("DISPLAY", std::env::var("DISPLAY").unwrap_or(":0".into()))
            .env("AGENTDECK_ASKPASS_ACCOUNT", format!("ssh-{}", s.id));
    } else {
        c.args(["-o", "BatchMode=yes"]);
    }
    c.arg(&s.host);
    Ok(c)
}
pub fn collector_binary() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("AGENTDECK_COLLECTOR") {
        return Ok(p.into());
    }
    let current = std::env::current_exe()?;
    if current.file_stem().and_then(|s| s.to_str()) == Some("agentdeck-collector") {
        return Ok(current);
    }
    let filename = if cfg!(windows) {
        "agentdeck-collector.exe"
    } else {
        "agentdeck-collector"
    };
    let next = current.parent().context("无程序目录")?.join(filename);
    anyhow::ensure!(next.exists(), "缺少随应用分发的采集器程序");
    Ok(next)
}
fn server(store: &Store, id: &str) -> Result<Server> {
    store
        .settings()?
        .servers
        .into_iter()
        .find(|s| s.id == id)
        .context("服务器不存在")
}
pub fn remote(store: &Store, s: &Server, method: &str, params: Value) -> Result<Value> {
    let request = json!({"version":1,"method":method,"params":params});
    let mut c = cmd(store, s)?;
    c.arg("$HOME/.local/lib/agentdeck/agentdeck-collector rpc");
    let output = process::run(
        &mut c,
        request.to_string().as_bytes(),
        if method == "quota" { 110 } else { 40 },
    )?;
    let v: Value =
        serde_json::from_slice(&output).context("服务器返回无效数据，请检查采集器安装")?;
    if let Some(e) = v["error"].as_str() {
        bail!("远端采集器：{e}");
    }
    anyhow::ensure!(v["version"] == 1, "采集器协议版本不兼容");
    Ok(v["result"].clone())
}
pub fn inspect(store: &Store, id: &str) -> Result<Value> {
    let s = server(store, id)?;
    let mut c = cmd(store, &s)?;
    c.arg(
        "uname -sm; command -v systemctl; systemctl --user is-system-running 2>/dev/null || true",
    );
    let out = process::run(&mut c, b"", 20)?;
    Ok(json!({"output":String::from_utf8_lossy(&out)}))
}

pub fn host_key(store: &Store, id: &str, accept: bool) -> Result<Value> {
    let s = server(store, id)?;
    validate(&s)?;
    if accept {
        let pending = store
            .get::<Value>(&format!("pending-key:{id}"))?
            .context("请先获取主机指纹")?;
        anyhow::ensure!(
            pending["expiresAt"].as_i64().unwrap_or(0) > now(),
            "指纹已过期，请重新检查"
        );
        anyhow::ensure!(
            pending["host"] == s.host && pending["port"] == json!(s.port),
            "连接配置已改变，请重新检查指纹"
        );
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(store.dir.join("known_hosts"))?;
        writeln!(f, "{}", pending["keys"].as_str().context("无主机密钥")?)?;
        store.set(&format!("pending-key:{id}"), &Value::Null)?;
        return Ok(json!({"trusted":true}));
    }
    let mut conf = Command::new("ssh");
    conf.arg("-G");
    if let Some(p) = s.port {
        conf.args(["-p", &p.to_string()]);
    }
    conf.arg(&s.host);
    let out = process::run(&mut conf, b"", 5)?;
    let cfg = String::from_utf8_lossy(&out);
    let value = |key: &str| {
        cfg.lines()
            .find_map(|l| l.strip_prefix(&format!("{key} ")))
            .map(str::to_string)
    };
    anyhow::ensure!(
        s.jump_host.is_empty() && value("proxyjump").is_none_or(|v| v == "none"),
        "跳板连接请先在终端运行 ssh 并核对主机指纹，再回到应用测试"
    );
    let hostname = value("hostname").unwrap_or(s.host.clone());
    let port = value("port").unwrap_or("22".into());
    anyhow::ensure!(
        safe_host(&hostname) && !hostname.starts_with('-'),
        "无法解析 SSH 主机"
    );
    let keys = process::run(
        Command::new("ssh-keyscan").args(["-T", "5", "-p", &port, &hostname]),
        b"",
        10,
    )?;
    let keys = String::from_utf8(keys)?;
    anyhow::ensure!(!keys.trim().is_empty(), "未获取主机密钥");
    let fingerprints = process::run(
        Command::new("ssh-keygen").args(["-l", "-f", "-"]),
        keys.as_bytes(),
        5,
    )?;
    store.set(
        &format!("pending-key:{id}"),
        &json!({"host":s.host,"port":s.port,"keys":keys,"expiresAt":now()+300}),
    )?;
    Ok(
        json!({"fingerprints":String::from_utf8_lossy(&fingerprints),"hostname":hostname,"port":port}),
    )
}

pub fn install(store: &Store, id: &str, binary_path: Option<&str>) -> Result<Value> {
    let s = server(store, id)?;
    let mut probe = cmd(store, &s)?;
    probe.arg("uname -sm");
    let arch = String::from_utf8(process::run(&mut probe, b"", 15)?)?;
    let target = match arch.trim() {
        "Linux x86_64" => "x86_64-unknown-linux-gnu",
        "Linux aarch64" => "aarch64-unknown-linux-gnu",
        _ => bail!("不支持的服务器系统或架构"),
    };
    let bin = if let Some(path) = binary_path.filter(|p| !p.is_empty()) {
        PathBuf::from(path)
    } else {
        let own = collector_binary()?;
        let bundled = RESOURCES
            .get()
            .map(PathBuf::as_path)
            .unwrap_or_else(|| own.parent().unwrap())
            .join("collectors")
            .join(target)
            .join("agentdeck-collector");
        if bundled.exists() {
            bundled
        } else if cfg!(target_os = "linux")
            && ((cfg!(target_arch = "x86_64") && target.starts_with("x86_64"))
                || (cfg!(target_arch = "aarch64") && target.starts_with("aarch64")))
        {
            own
        } else {
            bail!("缺少 {target} 采集器，请选择对应的 Linux 采集器文件")
        }
    };
    let bytes = std::fs::read(&bin).context("无法读取采集器文件")?;
    anyhow::ensure!(bytes.starts_with(b"\x7fELF"), "必须选择 Linux ELF 采集器");
    let mut upload = cmd(store, &s)?;
    upload.arg("umask 077; mkdir -p \"$HOME/.local/lib/agentdeck\" && cat > \"$HOME/.local/lib/agentdeck/agentdeck-collector.new\" && chmod 700 \"$HOME/.local/lib/agentdeck/agentdeck-collector.new\" && \"$HOME/.local/lib/agentdeck/agentdeck-collector.new\" version && mv \"$HOME/.local/lib/agentdeck/agentdeck-collector.new\" \"$HOME/.local/lib/agentdeck/agentdeck-collector\"");
    process::run(&mut upload, &bytes, 120)?;
    let unit = "[Unit]\nDescription=AgentDeck metrics and agent usage collector\nAfter=network.target\n\n[Service]\nType=simple\nExecStart=%h/.local/lib/agentdeck/agentdeck-collector daemon\nRestart=on-failure\nRestartSec=5\nUMask=0077\nNoNewPrivileges=true\n\n[Install]\nWantedBy=default.target\n";
    let mut setup = cmd(store, &s)?;
    setup.arg("umask 077; mkdir -p \"$HOME/.config/systemd/user\" && cat > \"$HOME/.config/systemd/user/agentdeck.service\" && systemctl --user daemon-reload && systemctl --user enable --now agentdeck.service && systemctl --user restart agentdeck.service && systemctl --user is-active agentdeck.service && loginctl show-user \"$(id -u)\" -p Linger --value");
    let out = String::from_utf8(process::run(&mut setup, unit.as_bytes(), 30)?)?;
    let persistent = out.lines().any(|l| l.trim() == "yes");
    Ok(
        json!({"installed":true,"persistent":persistent,"message":if persistent{"采集服务已启动，用户常驻已启用"}else{"服务已启动，但需管理员执行 loginctl enable-linger <用户名> 才能保证登出和重启后运行"},"probe":remote(store,&s,"capabilities",json!({}))?}),
    )
}
pub fn uninstall(store: &Store, id: &str, delete_data: bool) -> Result<Value> {
    let s = server(store, id)?;
    let mut c = cmd(store, &s)?;
    let mut command="systemctl --user disable --now agentdeck.service && rm -f \"$HOME/.config/systemd/user/agentdeck.service\" && systemctl --user daemon-reload && rm -f \"$HOME/.local/lib/agentdeck/agentdeck-collector\"".to_string();
    if delete_data {
        command.push_str(" && rm -rf -- \"${XDG_DATA_HOME:-$HOME/.local/share}/agentdeck\"");
    }
    c.arg(command);
    process::run(&mut c, b"", 30)?;
    Ok(json!({"removed":true,"dataDeleted":delete_data}))
}
pub fn remote_quota(store: &Store, a: &Account) -> Result<QuotaSnapshot> {
    let s = server(store, &a.server_id)?;
    let v = remote(
        store,
        &s,
        "quota",
        json!({"provider":a.provider,"executable":a.executable,"credentialPath":a.credential_path}),
    )?;
    let mut q: QuotaSnapshot = serde_json::from_value(v)?;
    q.id = a.id.clone();
    q.label = a.label.clone();
    q.device = s.id;
    Ok(q)
}

pub fn sync(store: &Store, id: &str) -> Result<Value> {
    let s = server(store, id)?;
    let result = (|| -> Result<Value> {
        let cap = remote(store, &s, "capabilities", json!({}))?;
        let settings = store.settings()?;
        let policy = json!({"rawHours":settings.raw_hours,"minuteDays":settings.minute_days,"hourDays":settings.hour_days});
        if store.get::<Value>(&format!("policy:{id}"))?.as_ref() != Some(&policy) {
            remote(store, &s, "configure", policy.clone())?;
            store.set(&format!("policy:{id}"), &policy)?;
        }
        let device = cap["device"].as_str().context("远端无设备标识")?;
        let mut cursor = store
            .get::<i64>(&format!("sync:{id}:{device}"))?
            .unwrap_or(0);
        let initial = remote(store, &s, "changes", json!({"after":cursor}))?;
        if cursor == 0 || initial["gap"] == true {
            let watermark = cap["watermark"].as_i64().unwrap_or(0);
            for kind in ["usage", "samples"] {
                let mut after = String::new();
                loop {
                    let page = remote(store, &s, "export", json!({"kind":kind,"after":after}))?;
                    let rows = page["rows"].as_array().context("无效同步页")?;
                    for row in rows {
                        import(store, kind, row)?;
                    }
                    if !page["hasMore"].as_bool().unwrap_or(false) {
                        break;
                    }
                    let next = page["cursor"].as_str().context("无同步游标")?;
                    anyhow::ensure!(next != after, "同步游标未前进");
                    after = next.into();
                }
            }
            cursor = watermark;
        }
        for _ in 0..200 {
            let page = remote(store, &s, "changes", json!({"after":cursor}))?;
            let rows = page["rows"].as_array().context("无效同步页")?;
            for row in rows {
                import(store, row["kind"].as_str().unwrap_or(""), &row["payload"])?;
            }
            let next = page["cursor"].as_i64().context("无同步游标")?;
            anyhow::ensure!(next >= cursor, "同步游标倒退");
            cursor = next;
            store.set(&format!("sync:{id}:{device}"), &cursor)?;
            if page["hasMore"] != true {
                break;
            }
        }
        let latest = remote(store, &s, "latest", json!({}))?;
        let state = json!({"id":id,"device":device,"label":s.label,"status":"ready","updatedAt":now(),"sample":latest,"collector":cap});
        store.set(&format!("server:{id}"), &state)?;
        Ok(state)
    })();
    match result {
        Ok(v) => Ok(v),
        Err(e) => {
            let mut v = store
                .get::<Value>(&format!("server:{id}"))?
                .unwrap_or(json!({"id":id,"label":s.label}));
            v["status"] = json!("error");
            v["error"] = json!(e.to_string());
            v["attemptedAt"] = json!(now());
            store.set(&format!("server:{id}"), &v)?;
            Ok(v)
        }
    }
}
fn import(store: &Store, kind: &str, payload: &Value) -> Result<()> {
    match kind {
        "usage" => {
            let e: UsageEvent = serde_json::from_value(payload.clone())?;
            store.put_events(&[e], false)?;
        }
        "sample" | "samples" => {
            store.sample(
                payload["device"].as_str().context("device")?,
                payload["timestamp"].as_i64().context("timestamp")?,
                payload["resolution"].as_i64().context("resolution")?,
                &payload["payload"],
                false,
            )?;
        }
        _ => bail!("不支持的同步记录类型"),
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_shell_options_and_injection() {
        for host in ["-oProxyCommand=bad", "x;touch /tmp/x", "$(id)", "x\ny"] {
            let s = Server {
                host: host.into(),
                id: "a".into(),
                ..Default::default()
            };
            assert!(validate(&s).is_err());
        }
        assert!(safe_host("user@host-name"));
        assert_eq!(quote("a'b"), "'a'\\''b'");
    }
}
