#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use agentdeck_core::{model::QuotaSnapshot, store::Store};
use serde_json::Value;
use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

#[tauri::command]
async fn api(
    store: tauri::State<'_, Store>,
    method: String,
    params: Value,
) -> Result<Value, String> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        agentdeck_core::dispatch(&store, &method, params).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            agentdeck_core::ssh::set_resources(app.path().resource_dir()?);
            let store = Store::open(Store::default_dir())?;
            app.manage(store.clone());
            let _ = agentdeck_core::start_desktop_workers(store.clone());
            let open = MenuItem::with_id(app, "open", "打开 AgentDeck", true, None::<&str>)?;
            let summary =
                MenuItem::with_id(app, "summary", "正在读取账户与服务器…", false, None::<&str>)?;
            let refresh = MenuItem::with_id(app, "refresh", "刷新额度", true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause", "暂停 / 恢复后台采集", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &summary, &refresh, &pause, &quit])?;
            let mut builder = TrayIconBuilder::with_id("main-tray")
                .menu(&menu)
                .tooltip("AgentDeck")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    "pause" => {
                        let s = app.state::<Store>();
                        if let Ok(mut settings) = s.settings() {
                            settings.paused = !settings.paused;
                            let _ = s.set("settings", &settings);
                        }
                    }
                    "refresh" => {
                        let s = app.state::<Store>().inner().clone();
                        std::thread::spawn(move || {
                            let _ = agentdeck_core::quota::refresh(&s, None);
                        });
                    }
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                builder = builder.icon(icon.clone());
            }
            let tray = builder.build(app)?;
            std::thread::spawn(move || {
                loop {
                    let quotas = store
                        .get::<Vec<QuotaSnapshot>>("quotas")
                        .ok()
                        .flatten()
                        .unwrap_or_default();
                    let mut parts = quotas
                        .iter()
                        .filter_map(|q| {
                            let suffix = if q.status == "ready" {
                                ""
                            } else {
                                " (旧快照)"
                            };
                            if let Some(b) = q
                                .balance
                                .as_ref()
                                .and_then(|b| b["balance_infos"].as_array())
                            {
                                Some(format!(
                                    "DeepSeek {}{}",
                                    b.iter()
                                        .map(|v| format!(
                                            "{} {}",
                                            v["currency"].as_str().unwrap_or(""),
                                            v["total_balance"].as_str().unwrap_or("—")
                                        ))
                                        .collect::<Vec<_>>()
                                        .join(" / "),
                                    suffix
                                ))
                            } else {
                                q.windows.first().map(|w| {
                                    format!(
                                        "{} 剩余 {:.0}%{}",
                                        q.provider,
                                        (100. - w.used_percent).max(0.),
                                        suffix
                                    )
                                })
                            }
                        })
                        .collect::<Vec<_>>();
                    if let Ok(settings) = store.settings() {
                        if !settings.servers.is_empty() {
                            let online = settings
                                .servers
                                .iter()
                                .filter(|s| {
                                    store
                                        .get::<Value>(&format!("server:{}", s.id))
                                        .ok()
                                        .flatten()
                                        .is_some_and(|v| {
                                            v["status"] == "ready"
                                                && agentdeck_core::model::now()
                                                    - v["updatedAt"].as_i64().unwrap_or(0)
                                                    < 120
                                        })
                                })
                                .count();
                            parts.push(format!("服务器 {online}/{} 在线", settings.servers.len()));
                        }
                        if settings.paused {
                            parts.push("已暂停".into());
                        }
                    }
                    let text = parts.join(" · ");
                    let _ = summary.set_text(if text.is_empty() {
                        "额度尚未获取"
                    } else {
                        &text
                    });
                    let _ = tray.set_tooltip(Some(format!("AgentDeck · {text}")));
                    std::thread::sleep(std::time::Duration::from_secs(15));
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![api])
        .run(tauri::generate_context!())
        .expect("AgentDeck failed to start");
}
