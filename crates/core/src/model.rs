use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
pub fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub sources: Vec<Source>,
    pub servers: Vec<Server>,
    pub accounts: Vec<Account>,
    pub theme: String,
    pub poll_seconds: u64,
    pub quota_seconds: u64,
    pub concurrency: usize,
    pub paused: bool,
    pub price_overrides: Vec<PriceOverride>,
    pub raw_hours: i64,
    pub minute_days: i64,
    pub hour_days: i64,
}
impl Default for Settings {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        Self {
            sources: vec![
                Source {
                    id: "local-codex".into(),
                    agent: "codex".into(),
                    path: home.join(".codex/sessions").to_string_lossy().into(),
                    enabled: true,
                },
                Source {
                    id: "local-claude".into(),
                    agent: "claude".into(),
                    path: home.join(".claude/projects").to_string_lossy().into(),
                    enabled: true,
                },
                Source {
                    id: "local-agy".into(),
                    agent: "agy".into(),
                    path: home.join(".gemini").to_string_lossy().into(),
                    enabled: true,
                },
            ],
            servers: vec![],
            accounts: vec![
                Account {
                    id: "local-codex".into(),
                    provider: "codex".into(),
                    label: "Codex · 本机".into(),
                    ..Default::default()
                },
                Account {
                    id: "local-claude".into(),
                    provider: "claude".into(),
                    label: "Claude Code · 本机".into(),
                    ..Default::default()
                },
                Account {
                    id: "local-agy".into(),
                    provider: "agy".into(),
                    label: "Antigravity · 本机".into(),
                    ..Default::default()
                },
            ],
            theme: "system".into(),
            poll_seconds: 60,
            quota_seconds: 300,
            concurrency: 4,
            paused: false,
            price_overrides: vec![],
            raw_hours: 48,
            minute_days: 30,
            hour_days: 365,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub agent: String,
    pub path: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Server {
    pub id: String,
    pub label: String,
    pub host: String,
    pub port: Option<u16>,
    pub identity_file: String,
    pub jump_host: String,
    pub selected_disks: Vec<String>,
    pub selected_interfaces: Vec<String>,
    pub selected_gpus: Vec<String>,
    pub hidden_metrics: Vec<String>,
    pub order: i32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub provider: String,
    pub label: String,
    pub server_id: String,
    pub executable: String,
    pub credential_path: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PriceOverride {
    pub model: String,
    pub catalog_id: String,
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageEvent {
    pub id: String,
    pub source: String,
    pub device: String,
    pub agent: String,
    pub session: String,
    pub timestamp: i64,
    pub model: String,
    pub input: i64,
    pub output: i64,
    pub cache_read: Option<i64>,
    pub cache_write: Option<i64>,
    pub reasoning: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub source: String,
    pub status: String,
    pub scanned_at: i64,
    pub events: usize,
    pub errors: usize,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct QuotaSnapshot {
    pub id: String,
    pub provider: String,
    pub label: String,
    pub account_id: Option<String>,
    pub device: String,
    pub windows: Vec<QuotaWindow>,
    pub reset_cards: Option<Value>,
    pub balance: Option<Value>,
    pub updated_at: Option<i64>,
    pub attempted_at: i64,
    pub status: String,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub id: String,
    pub label: String,
    pub used_percent: f64,
    pub resets_at: Option<i64>,
    pub duration_minutes: Option<i64>,
}

pub fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 80
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
