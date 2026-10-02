use anyhow::{Context, Result, bail};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub fn executable(name: &str, configured: &str) -> String {
    if !configured.is_empty() {
        return configured.into();
    }
    let mut dirs: Vec<std::path::PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = dirs::home_dir() {
        dirs.extend([home.join(".local/bin"), home.join(".cargo/bin")]);
    }
    dirs.extend([
        std::path::PathBuf::from("/opt/homebrew/bin"),
        std::path::PathBuf::from("/usr/local/bin"),
    ]);
    #[cfg(windows)]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(std::path::PathBuf::from(appdata).join("npm"));
    }
    for dir in dirs {
        for suffix in if cfg!(windows) {
            &[".exe", ".cmd", ".bat", ""][..]
        } else {
            &[""][..]
        } {
            let path = dir.join(format!("{name}{suffix}"));
            if path.is_file() {
                return path.to_string_lossy().into();
            }
        }
    }
    name.into()
}

pub fn run(command: &mut Command, input: &[u8], timeout: u64) -> Result<Vec<u8>> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().context("无法启动程序，请检查安装和路径")?;
    let mut stdin = child.stdin.take().unwrap();
    let owned = input.to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&owned);
    });
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut v = vec![];
        let _ = stdout.take(16 * 1024 * 1024 + 1).read_to_end(&mut v);
        v
    });
    let err = std::thread::spawn(move || {
        let mut v = vec![];
        let _ = stderr.take(1024 * 1024).read_to_end(&mut v);
        v
    });
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break Some(s);
        }
        if start.elapsed() > Duration::from_secs(timeout) {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            #[cfg(windows)]
            {
                let _ = Command::new("taskkill")
                    .args(["/PID", &child.id().to_string(), "/T", "/F"])
                    .output();
            }
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(30));
    };
    let _ = writer.join();
    let output = out.join().unwrap_or_default();
    let error = err.join().unwrap_or_default();
    let Some(status) = status else {
        bail!("查询超时（{timeout} 秒），已终止本次进程")
    };
    if !status.success() {
        let e = String::from_utf8_lossy(&error);
        let category = if e.contains("Host key verification failed")
            || e.contains("REMOTE HOST IDENTIFICATION HAS CHANGED")
        {
            "SSH 主机密钥未信任或发生变化"
        } else if e.contains("Permission denied") {
            "SSH 身份验证失败"
        } else if e.contains("Connection refused") {
            "SSH 连接被拒绝"
        } else {
            "程序返回错误；请检查安装、登录状态及连接"
        };
        bail!("{category} ({})", status.code().unwrap_or(-1));
    }
    anyhow::ensure!(output.len() <= 16 * 1024 * 1024, "查询输出超出大小限制");
    Ok(output)
}

/// Bounded stdio RPC; only our child process is ever terminated.
pub fn codex_rpc(executable: &str) -> Result<serde_json::Value> {
    use serde_json::{Value, json};
    use std::{
        io::{BufRead, BufReader},
        sync::mpsc,
    };
    let mut cmd = Command::new(executable);
    cmd.arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let mut child = cmd.spawn().context("无法启动 Codex App Server")?;
    let mut input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let r = BufReader::new(output);
        for line in r.lines() {
            let Ok(line) = line else { break };
            if line.len() > 1024 * 1024 {
                break;
            }
            if let Ok(v) = serde_json::from_str::<Value>(&line)
                && tx.send(v).is_err()
            {
                break;
            }
        }
    });
    let result = (|| -> Result<Value> {
        writeln!(
            input,
            "{}",
            json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"agentdeck","version":"0.1.0"}}})
        )?;
        input.flush()?;
        let deadline = Instant::now() + Duration::from_secs(30);
        let receive = |id: i64| -> Result<Value> {
            loop {
                let v = rx
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .context("Codex 查询超时")?;
                if v["id"] == id {
                    if !v["error"].is_null() {
                        bail!("Codex 接口拒绝请求，请检查登录与版本")
                    };
                    return Ok(v["result"].clone());
                }
            }
        };
        receive(1)?;
        writeln!(input, "{}", json!({"method":"initialized"}))?;
        writeln!(
            input,
            "{}",
            json!({"id":2,"method":"account/read","params":{"refreshToken":false}})
        )?;
        input.flush()?;
        let account = receive(2)?;
        writeln!(
            input,
            "{}",
            json!({"id":3,"method":"account/rateLimits/read"})
        )?;
        input.flush()?;
        let quota = receive(3)?;
        Ok(json!({"account":account,"quota":quota}))
    })();
    drop(input);
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &child.id().to_string()])
            .creation_flags(0x08000000)
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    result
}
