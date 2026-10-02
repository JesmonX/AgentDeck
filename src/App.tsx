import { useCallback, useEffect, useState } from "react";
import {
  LayoutDashboard,
  Server,
  Settings as SettingsIcon,
  Command,
  ArrowUpRight,
  Sun,
  Moon,
  PanelLeftClose,
  RefreshCw,
  CheckCircle2,
  AlertCircle,
  Pause,
  Play,
} from "lucide-react";
import type { Snapshot, Settings as SettingsType } from "./types";
import { api, snapshot, isDemo } from "./api";
import { Agents } from "./Agents";
import { Servers } from "./Servers";
import { Settings } from "./Settings";
import { Busy, Empty } from "./components";
export default function App() {
  const [data, setData] = useState<Snapshot | null>(null),
    [page, setPage] = useState("agents"),
    [settingsTab, setSettingsTab] = useState("general"),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(0),
    [toast, setToast] = useState(""),
    [collapsed, setCollapsed] = useState(false),
    [theme, setTheme] = useState(
      localStorage.getItem("agentdeck-theme") ?? "system",
    );
  const load = useCallback(async () => {
    try {
      setData(await snapshot());
      setError("");
    } catch (e) {
      setError(String(e));
    }
  }, []);
  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      await load();
      if (!disposed) timer = setTimeout(poll, 15000);
    };
    poll();
    return () => {
      disposed = true;
      clearTimeout(timer);
    };
  }, [load]);
  useEffect(() => {
    if (data) setTheme(data.settings.theme);
  }, [data?.settings.theme]);
  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      document.documentElement.dataset.theme =
        theme === "system" ? (media.matches ? "dark" : "light") : theme;
    };
    apply();
    localStorage.setItem("agentdeck-theme", theme);
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(""), 4500);
    return () => clearTimeout(t);
  }, [toast]);
  const run = async (method: string, params: unknown = {}) => {
    setBusy((n) => n + 1);
    try {
      const result = await api(method, params);
      await load();
      setToast("已刷新，请查看各项数据状态");
      return result;
    } catch (e) {
      setError(String(e));
      return null;
    } finally {
      setBusy((n) => n - 1);
    }
  };
  const save = async (settings: SettingsType) => {
    await api("saveSettings", settings);
    setTheme(settings.theme);
    await load();
  };
  const openSettings = (tab: string) => {
    setSettingsTab(tab);
    setPage("settings");
  };
  const toggleTheme = () => {
    const next =
      document.documentElement.dataset.theme === "dark" ? "light" : "dark";
    setTheme(next);
    if (data)
      save({ ...data.settings, theme: next }).catch((e) => setError(String(e)));
  };
  return (
    <div className={`app-shell ${collapsed ? "collapsed" : ""}`}>
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">
            <Command size={22} />
          </div>
          <span>
            AgentDeck<small>YOUR PERSONAL OBSERVATORY</small>
          </span>
        </div>
        <div className="nav-label">工作空间</div>
        <nav>
          {[
            { id: "agents", name: "Agent 概览", icon: LayoutDashboard },
            { id: "servers", name: "服务器", icon: Server },
            { id: "settings", name: "设置", icon: SettingsIcon },
          ].map(({ id, name, icon: Icon }) => (
            <button
              key={id}
              title={name}
              className={page === id ? "active" : ""}
              onClick={() => setPage(id)}
            >
              <Icon size={19} />
              <span>{name}</span>
              {id === "servers" && !!data?.settings.servers.length && (
                <b>{data.settings.servers.length}</b>
              )}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="local-card">
            <span className="local-icon">
              <CheckCircle2 size={19} />
            </span>
            <div>
              <strong>本地优先</strong>
              <p>数据留在你的设备</p>
            </div>
          </div>
          <div className="sidebar-footer">
            <span>
              AgentDeck <small>v0.1.0</small>
            </span>
            <button
              className="icon-button"
              onClick={() => setCollapsed(!collapsed)}
              aria-label="收起侧栏"
            >
              <PanelLeftClose size={17} />
            </button>
          </div>
        </div>
      </aside>
      <div className="main-shell">
        <header className="topbar">
          <div className="breadcrumb">
            工作空间<span>/</span>
            <b>
              {page === "agents"
                ? "Agent 概览"
                : page === "servers"
                  ? "服务器"
                  : "设置"}
            </b>
          </div>
          <div className="topbar-actions">
            {isDemo && <span className="demo-badge">演示数据</span>}
            <span className="connection">
              <i className={error ? "stale" : "live"} />
              {busy ? (
                <Busy>更新中</Busy>
              ) : data?.settings.paused ? (
                "采集已暂停"
              ) : data ? (
                "本地服务已连接"
              ) : (
                "连接本地服务"
              )}
            </span>
            {data && (
              <button
                className="icon-button"
                aria-label={data.settings.paused ? "恢复采集" : "暂停采集"}
                onClick={() =>
                  save({
                    ...data.settings,
                    paused: !data.settings.paused,
                  }).catch((e) => setError(String(e)))
                }
              >
                {data.settings.paused ? (
                  <Play size={16} />
                ) : (
                  <Pause size={16} />
                )}
              </button>
            )}
            <button
              className="icon-button"
              aria-label="切换主题"
              onClick={toggleTheme}
            >
              {theme === "dark" ? <Sun size={18} /> : <Moon size={18} />}
            </button>
            <div className="avatar">AD</div>
          </div>
        </header>
        <main>
          {error && (
            <div className="notice error" role="alert">
              <AlertCircle size={16} />
              <span>{error}</span>
              <button className="text-button" onClick={load}>
                <RefreshCw size={13} />
                重试
              </button>
            </div>
          )}
          {data ? (
            page === "agents" ? (
              <Agents
                data={data}
                run={run}
                openSettings={() => openSettings("sources")}
              />
            ) : page === "servers" ? (
              <Servers
                data={data}
                run={run}
                settings={() => openSettings("servers")}
                saveSettings={save}
              />
            ) : (
              <Settings
                key={settingsTab}
                data={data}
                save={save}
                run={run}
                initialTab={settingsTab}
              />
            )
          ) : error ? (
            <Empty
              title="启动你的个人观测台"
              detail="请在项目目录运行 pnpm dev，或打开 AgentDeck 桌面应用。"
              action={
                <a className="button" href="?demo=1">
                  查看界面演示
                  <ArrowUpRight size={15} />
                </a>
              }
            />
          ) : (
            <div className="loading-page">
              <div className="brand-mark">
                <Command size={28} />
              </div>
              <h1>AgentDeck</h1>
              <Busy>正在读取本地数据…</Busy>
            </div>
          )}
        </main>
        <footer className="app-footer">
          <span>AgentDeck · 让工作与资源清晰可见</span>
          <span>
            {isDemo ? "界面演示 · 非真实账户数据" : "本地存储 · SSH 加密传输"}
          </span>
        </footer>
      </div>
      {toast && (
        <div className="toast" role="status">
          <CheckCircle2 size={17} />
          {toast}
        </div>
      )}
    </div>
  );
}
