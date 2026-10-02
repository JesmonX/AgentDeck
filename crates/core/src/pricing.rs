use crate::{model::*, store::Store};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::collections::HashMap;

pub fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(25))
        .user_agent("AgentDeck/0.1.0")
        .build()?)
}
pub fn refresh(store: &Store) -> Result<Value> {
    let response = client()?
        .get("https://openrouter.ai/api/v1/models")
        .send()
        .context("价格目录连接失败")?;
    anyhow::ensure!(
        response.status().is_success(),
        "价格目录返回 HTTP {}",
        response.status().as_u16()
    );
    let v: Value = response.json()?;
    anyhow::ensure!(v["data"].is_array(), "无效价格目录");
    let result = json!({"updatedAt":now(),"data":v["data"]});
    store.set("prices", &result)?;
    Ok(json!({"updatedAt":now(),"models":v["data"].as_array().map(Vec::len)}))
}

pub struct Estimator {
    catalog: HashMap<String, Value>,
    overrides: Vec<PriceOverride>,
}
fn index_catalog(models: Vec<Value>) -> HashMap<String, Value> {
    let mut index = HashMap::new();
    let mut suffixes: HashMap<String, Option<Value>> = HashMap::new();
    for model in models {
        if let Some(id) = model["id"].as_str() {
            index.insert(id.to_string(), model.clone());
            if let Some((_, suffix)) = id.split_once('/') {
                suffixes
                    .entry(suffix.to_string())
                    .and_modify(|v| *v = None)
                    .or_insert(Some(model));
            }
        }
    }
    for (suffix, value) in suffixes {
        if let Some(model) = value {
            index.entry(suffix).or_insert(model);
        }
    }
    index
}
impl Estimator {
    pub fn load(store: &Store) -> Result<Self> {
        let catalog = store.get::<Value>("prices")?.unwrap_or(Value::Null)["data"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(Self {
            catalog: index_catalog(catalog),
            overrides: store.settings()?.price_overrides,
        })
    }
    pub fn estimate(&self, e: &UsageEvent) -> Option<f64> {
        let manual = self.overrides.iter().find(|x| x.model == e.model);
        let target = manual
            .filter(|x| !x.catalog_id.is_empty())
            .map(|x| x.catalog_id.as_str())
            .unwrap_or(&e.model);
        let matched = self.catalog.get(target);
        let mut price = matched.map(|m| m["pricing"].clone()).unwrap_or(json!({}));
        if let Some(overrides) = price["overrides"].as_array().cloned() {
            use chrono::{Datelike, Timelike};
            let dt = chrono::DateTime::from_timestamp(e.timestamp, 0)?;
            for rule in overrides {
                if rule["min_prompt_tokens"]
                    .as_i64()
                    .is_some_and(|n| e.input <= n)
                {
                    continue;
                }
                if let (Some(start), Some(end)) =
                    (rule["utc_start"].as_u64(), rule["utc_end"].as_u64())
                {
                    let hhmm = (dt.hour() * 100 + dt.minute()) as u64;
                    let matches = if end > start {
                        hhmm >= start && hhmm < end
                    } else {
                        hhmm >= start || hhmm < end
                    };
                    if !matches {
                        continue;
                    }
                }
                if let Some(days) = rule["utc_days"].as_array() {
                    let names = [
                        "monday",
                        "tuesday",
                        "wednesday",
                        "thursday",
                        "friday",
                        "saturday",
                        "sunday",
                    ];
                    if !days
                        .iter()
                        .any(|d| d == names[dt.weekday().num_days_from_monday() as usize])
                    {
                        continue;
                    }
                }
                for key in [
                    "prompt",
                    "completion",
                    "input_cache_read",
                    "input_cache_write",
                ] {
                    if !rule[key].is_null() {
                        price[key] = rule[key].clone();
                    }
                }
            }
        }
        let get = |key: &str, custom: Option<f64>| {
            custom
                .map(|x| x / 1_000_000.)
                .or_else(|| {
                    price[key]
                        .as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .or(price[key].as_f64())
                })
                .filter(|v| v.is_finite() && *v >= 0.)
        };
        let p = get("prompt", manual.and_then(|m| m.input))?;
        let o = get("completion", manual.and_then(|m| m.output))?;
        let read = e.cache_read.unwrap_or(0);
        let write = e.cache_write.unwrap_or(0);
        let uncached = e.input.checked_sub(read)?.checked_sub(write)?;
        if uncached < 0 {
            return None;
        }
        let cr = if read == 0 {
            0.
        } else {
            get("input_cache_read", manual.and_then(|m| m.cache_read))?
        };
        let cw = if write == 0 {
            0.
        } else {
            get("input_cache_write", manual.and_then(|m| m.cache_write))?
        };
        Some(uncached as f64 * p + e.output as f64 * o + read as f64 * cr + write as f64 * cw)
    }
}
pub fn estimate(store: &Store, e: &UsageEvent) -> Result<Option<f64>> {
    Ok(Estimator::load(store)?.estimate(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ambiguous_model_suffix_is_never_assigned_a_provider_price() {
        let index = index_catalog(vec![
            json!({"id":"a/shared"}),
            json!({"id":"b/shared"}),
            json!({"id":"a/unique"}),
        ]);
        assert!(!index.contains_key("shared"));
        assert!(index.contains_key("a/shared"));
        assert!(index.contains_key("unique"));
    }
    #[test]
    fn unknown_not_free_and_cache_not_double_charged() {
        let p = Estimator {
            catalog: index_catalog(vec![
                json!({"id":"x/m","pricing":{"prompt":"0.000002","completion":"0.00001","input_cache_read":"0.000001"}}),
            ]),
            overrides: vec![],
        };
        let mut e = UsageEvent {
            model: "m".into(),
            input: 100,
            output: 10,
            cache_read: Some(50),
            ..Default::default()
        };
        assert!((p.estimate(&e).unwrap() - 0.00025).abs() < 1e-10);
        e.model = "unknown".into();
        assert!(p.estimate(&e).is_none());
    }
}
