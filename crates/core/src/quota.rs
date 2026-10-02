use crate::{model::*, pricing, process, store::Store};
use anyhow::{Context, Result, bail};
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::{path::PathBuf, process::Command};

pub fn set_secret(account: &str, value: &str) -> Result<()> {
    anyhow::ensure!(valid_id(account), "账户 ID 无效");
    let e = keyring::Entry::new("AgentDeck", account)?;
    if value.is_empty() {
        match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(err) => return Err(err.into()),
        }
    } else {
        e.set_password(value)
            .context("系统凭据库不可用；未保存密钥")?;
    }
    Ok(())
}
fn secret(account: &str) -> Result<String> {
    keyring::Entry::new("AgentDeck", account)?
        .get_password()
        .context("未配置 API Key，或系统凭据库锁定")
}
fn timestamp(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| {
        v.as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|t| t.timestamp())
    })
}
fn http_json(response: reqwest::blocking::Response) -> Result<Value> {
    let status = response.status();
    if !status.is_success() {
        bail!(
            "账户查询 HTTP {}：{}",
            status.as_u16(),
            match status.as_u16() {
                401 | 403 => "凭据失效或权限不足，请重新登录",
                429 => "请求受限，稍后重试",
                _ => "服务暂不可用",
            }
        );
    }
    response.json().context("接口响应格式不兼容")
}

pub fn fetch(account: &Account) -> Result<QuotaSnapshot> {
    let mut s = QuotaSnapshot {
        id: account.id.clone(),
        provider: account.provider.clone(),
        label: account.label.clone(),
        device: if account.server_id.is_empty() {
            "本机".into()
        } else {
            account.server_id.clone()
        },
        attempted_at: now(),
        ..Default::default()
    };
    match account.provider.as_str() {
        "codex" => {
            let v = process::codex_rpc(&process::executable("codex", &account.executable))?;
            s.account_id = v
                .pointer("/account/account/email")
                .and_then(Value::as_str)
                .map(str::to_string);
            let quota = &v["quota"];
            let buckets: Vec<(String, Value)> =
                if let Some(map) = quota["rateLimitsByLimitId"].as_object() {
                    map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                } else {
                    vec![("codex".into(), quota["rateLimits"].clone())]
                };
            for (name, bucket) in buckets {
                for slot in ["primary", "secondary"] {
                    let w = &bucket[slot];
                    if let Some(percent) = w["usedPercent"].as_f64() {
                        let mins = w["windowDurationMins"].as_i64();
                        s.windows.push(QuotaWindow {
                            id: format!("{name}:{slot}"),
                            label: format!("{} · {}", name, window_label(mins)),
                            used_percent: percent,
                            resets_at: timestamp(&w["resetsAt"]),
                            duration_minutes: mins,
                        });
                    }
                }
            }
            s.reset_cards = quota
                .get("rateLimitResetCredits")
                .filter(|v| !v.is_null())
                .cloned();
        }
        "claude" => {
            let path = if account.credential_path.is_empty() {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(".claude/.credentials.json")
            } else {
                PathBuf::from(&account.credential_path)
            };
            let creds = std::fs::read_to_string(path).ok();
            #[cfg(target_os = "macos")]
            let creds = creds.or_else(|| {
                let out = process::run(
                    Command::new("security").args([
                        "find-generic-password",
                        "-s",
                        "Claude Code-credentials",
                        "-w",
                    ]),
                    b"",
                    10,
                )
                .ok()?;
                String::from_utf8(out).ok()
            });
            let v: Value = serde_json::from_str(
                &creds.context("未找到 Claude 登录凭据，请先运行 claude /login")?,
            )?;
            let token = v["claudeAiOauth"]["accessToken"]
                .as_str()
                .context("Claude 凭据缺少额度查询令牌")?;
            let client = pricing::client()?;
            let usage = http_json(
                client
                    .get("https://api.anthropic.com/api/oauth/usage")
                    .bearer_auth(token)
                    .header("anthropic-beta", "oauth-2025-04-20")
                    .send()
                    .context("Claude 额度连接失败")?,
            )?;
            if let Ok(response) = client
                .get("https://api.anthropic.com/api/oauth/profile")
                .bearer_auth(token)
                .header("anthropic-beta", "oauth-2025-04-20")
                .send()
                && let Ok(p) = http_json(response)
            {
                s.account_id = p
                    .pointer("/account/uuid")
                    .or(p.get("uuid"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            for (key, label, mins) in [
                ("five_hour", "5 小时", 300),
                ("seven_day", "7 天", 10080),
                ("seven_day_sonnet", "Sonnet · 7 天", 10080),
                ("seven_day_opus", "Opus · 7 天", 10080),
            ] {
                let w = &usage[key];
                if let Some(p) = w["utilization"].as_f64() {
                    s.windows.push(QuotaWindow {
                        id: key.into(),
                        label: label.into(),
                        used_percent: p,
                        resets_at: timestamp(&w["resets_at"]),
                        duration_minutes: Some(mins),
                    });
                }
            }
        }
        "agy" => {
            let output = process::run(
                Command::new(process::executable("agy", &account.executable)).args([
                    "-p",
                    "/usage",
                    "--output-format",
                    "json",
                ]),
                b"",
                90,
            )?;
            let v: Value = serde_json::from_slice(&output)
                .context("Antigravity 未返回 JSON 用量报告；请升级 CLI")?;
            s.windows = parse_agy(&v)?;
            s.account_id = v["account"]["email"]
                .as_str()
                .or(v["email"].as_str())
                .map(str::to_string);
        }
        "deepseek" => {
            let token = secret(&account.id)?;
            let v = http_json(
                pricing::client()?
                    .get("https://api.deepseek.com/user/balance")
                    .bearer_auth(token)
                    .send()
                    .context("DeepSeek 余额连接失败")?,
            )?;
            anyhow::ensure!(v["balance_infos"].is_array(), "DeepSeek 余额格式不兼容");
            s.balance = Some(v);
        }
        _ => bail!("不支持的账户类型"),
    }
    anyhow::ensure!(
        !s.windows.is_empty() || s.reset_cards.is_some() || s.balance.is_some(),
        "账户未返回可用额度窗口"
    );
    s.status = "ready".into();
    s.updated_at = Some(now());
    Ok(s)
}
fn window_label(mins: Option<i64>) -> String {
    match mins {
        Some(300) => "5 小时".into(),
        Some(10080) => "7 天".into(),
        Some(n) => format!("{n} 分钟"),
        None => "额度窗口".into(),
    }
}

pub fn parse_agy(v: &Value) -> Result<Vec<QuotaWindow>> {
    if let Some(groups) = v.pointer("/command/data/groups").and_then(Value::as_array) {
        anyhow::ensure!(
            v.pointer("/command/name").and_then(Value::as_str) == Some("usage")
                && v["status"] == "SUCCESS",
            "Antigravity 用量命令未成功"
        );
        let mut windows = vec![];
        for group in groups {
            let name = group["name"].as_str().unwrap_or("模型组");
            if let Some(buckets) = group["buckets"].as_array() {
                for bucket in buckets {
                    let Some(remaining) = bucket["remaining_fraction"]
                        .as_f64()
                        .filter(|n| (0.0..=1.0).contains(n))
                    else {
                        continue;
                    };
                    let mins = match bucket["window"].as_str() {
                        Some("5h") => Some(300),
                        Some("weekly") => Some(10080),
                        _ => None,
                    };
                    windows.push(QuotaWindow {
                        id: format!("{name}:{}", bucket["id"].as_str().unwrap_or("quota")),
                        label: format!("{name} · {}", window_label(mins)),
                        used_percent: (1. - remaining) * 100.,
                        resets_at: timestamp(&bucket["reset_time"]),
                        duration_minutes: mins,
                    });
                }
            }
        }
        anyhow::ensure!(!windows.is_empty(), "Antigravity 用量报告缺少实测额度");
        return Ok(windows);
    }
    // Explicit measured windows only: model availability is never interpreted as 100% quota.
    fn visit(v: &Value, path: &str, out: &mut Vec<QuotaWindow>, depth: usize) {
        if depth > 12 {
            return;
        }
        if let Some(obj) = v.as_object() {
            let percent = obj
                .get("usedPercent")
                .or(obj.get("used_percentage"))
                .or(obj.get("utilization"))
                .and_then(Value::as_f64)
                .or_else(|| {
                    obj.get("remainingFraction")
                        .and_then(Value::as_f64)
                        .map(|n| (1. - n) * 100.)
                });
            let reset = obj
                .get("resetsAt")
                .or(obj.get("resets_at"))
                .or(obj.get("resetTime"))
                .and_then(timestamp);
            let duration = obj
                .get("windowDurationMins")
                .or(obj.get("durationMinutes"))
                .and_then(Value::as_i64)
                .or_else(|| {
                    if path.contains("five_hour") || path.contains("fiveHour") {
                        Some(300)
                    } else if path.contains("seven_day") || path.contains("weekly") {
                        Some(10080)
                    } else {
                        None
                    }
                });
            if let Some(p) = percent.filter(|p| p.is_finite() && *p >= 0.)
                && (reset.is_some() || duration.is_some())
            {
                out.push(QuotaWindow {
                    id: path.into(),
                    label: obj
                        .get("displayName")
                        .or(obj.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| path.trim_start_matches('.').to_string()),
                    used_percent: p,
                    resets_at: reset,
                    duration_minutes: duration,
                });
                return;
            }
            for (k, val) in obj {
                visit(val, &format!("{path}.{k}"), out, depth + 1);
            }
        } else if let Some(arr) = v.as_array() {
            for (i, val) in arr.iter().enumerate() {
                visit(val, &format!("{path}.{i}"), out, depth + 1);
            }
        } else if let Some(s) = v.as_str()
            && s.starts_with('{')
            && let Ok(v) = serde_json::from_str::<Value>(s)
        {
            visit(&v, path, out, depth + 1);
        }
    }
    let mut out = vec![];
    visit(v, "", &mut out, 0);
    anyhow::ensure!(
        !out.is_empty(),
        "Antigravity 未返回可识别的实测额度，请检查 CLI 版本和登录状态"
    );
    Ok(out)
}
pub fn refresh(store: &Store, id: Option<&str>) -> Result<Vec<QuotaSnapshot>> {
    static REFRESH: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _refresh = REFRESH
        .lock()
        .map_err(|_| anyhow::anyhow!("额度刷新锁不可用"))?;
    let settings = store.settings()?;
    let mut snapshots = store
        .get::<Vec<QuotaSnapshot>>("quotas")?
        .unwrap_or_default();
    for account in settings
        .accounts
        .iter()
        .filter(|a| id.is_none_or(|id| a.id == id))
    {
        let fetched = if account.server_id.is_empty() {
            fetch(account)
        } else {
            crate::ssh::remote_quota(store, account)
        };
        let _commit = crate::CONFIG_WRITE
            .lock()
            .map_err(|_| anyhow::anyhow!("配置锁不可用"))?;
        if !store
            .settings()?
            .accounts
            .iter()
            .any(|a| serde_json::to_value(a).ok() == serde_json::to_value(account).ok())
        {
            continue;
        }
        snapshots = store
            .get::<Vec<QuotaSnapshot>>("quotas")?
            .unwrap_or_default();
        let prior = snapshots.iter().position(|s| s.id == account.id);
        let current = match fetched {
            Ok(s) => s,
            Err(e) => {
                let mut s = prior
                    .map(|i| snapshots[i].clone())
                    .unwrap_or(QuotaSnapshot {
                        id: account.id.clone(),
                        provider: account.provider.clone(),
                        label: account.label.clone(),
                        ..Default::default()
                    });
                s.status = "error".into();
                s.error = Some(e.to_string());
                s.attempted_at = now();
                s
            }
        };
        if let Some(i) = prior {
            snapshots[i] = current
        } else {
            snapshots.push(current)
        };
        store.set("quotas", &snapshots)?;
    }
    Ok(snapshots)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn agy_never_guesses_model_availability() {
        assert!(parse_agy(&json!({"models":[{"name":"gemini","available":true}]})).is_err());
        let v = parse_agy(&json!({"weekly":{"utilization":42,"resets_at":1900000000}})).unwrap();
        assert_eq!(v[0].duration_minutes, Some(10080));
    }
}
