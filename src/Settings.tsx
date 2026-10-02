import { useState } from "react";
import {
  Plus,
  Trash2,
  Save,
  KeyRound,
  ShieldCheck,
  Terminal,
  RefreshCw,
  Download,
  Check,
  FolderOpen,
} from "lucide-react";
import type {
  Snapshot,
  Settings as SettingsType,
  Account,
  Server,
  Source,
  PriceOverride,
} from "./types";
import { agents, ago } from "./data";
import { Panel, Field, Modal, Status, AgentIcon, Busy } from "./components";
import { api, isDemo } from "./api";
const uid = () => crypto.randomUUID().slice(0, 8);
export function Settings({
  data,
  save,
  run,
  initialTab = "general",
}: {
  data: Snapshot;
  save: (s: SettingsType) => Promise<void>;
  run: (method: string, p?: unknown) => Promise<unknown>;
  initialTab?: string;
}) {
  const [tab, setTab] = useState(initialTab),
    [draft, setDraft] = useState(data.settings),
    [saving, setSaving] = useState(false),
    [saved, setSaved] = useState(false),
    [error, setError] = useState("");
  const [account, setAccount] = useState<Account | null>(null),
    [server, setServer] = useState<Server | null>(null),
    [source, setSource] = useState<Source | null>(null),
    [price, setPrice] = useState<PriceOverride | null>(null),
    [secret, setSecret] = useState(""),
    [keyFor, setKeyFor] = useState<string | null>(null),
    [confirm, setConfirm] = useState<{
      title: string;
      detail: string;
      action: () => Promise<void>;
    } | null>(null),
    [actionResult, setActionResult] = useState(""),
    [actionBusy, setActionBusy] = useState(false),
    [binaryPath, setBinaryPath] = useState("");
  async function persist(next = draft) {
    setSaving(true);
    setError("");
    try {
      await save(next);
      setDraft(next);
      setSaved(true);
      setTimeout(() => setSaved(false), 2500);
    } catch (e) {
      setError(String(e));
      throw e;
    } finally {
      setSaving(false);
    }
  }
  async function action(method: string, p: unknown) {
    setActionBusy(true);
    setError("");
    try {
      const result = await api<Record<string, unknown>>(method, p);
      setActionResult(
        typeof result.message === "string"
          ? result.message
          : JSON.stringify(result, null, 2),
      );
      return result;
    } catch (e) {
      setError(String(e));
      return null;
    } finally {
      setActionBusy(false);
    }
  }
  const tabs = [
    ["general", "通用"],
    ["sources", "数据源"],
    ["accounts", "账户与额度"],
    ["servers", "SSH 服务器"],
    ["prices", "模型定价"],
  ];
  return (
    <>
      <div className="page-heading">
        <div>
          <div className="eyebrow">MAKE IT YOURS</div>
          <h1>
            设置
            <span className="heading-dot" />
          </h1>
          <p>连接你的工具，让数据各归其位。</p>
        </div>
        <button
          className="button primary"
          disabled={saving}
          onClick={() => persist().catch(() => {})}
        >
          {saving ? <Busy /> : saved ? <Check size={15} /> : <Save size={15} />}{" "}
          {saved ? "已保存" : "保存设置"}
        </button>
      </div>
      <div className="settings-tabs">
        {tabs.map(([id, label]) => (
          <button
            key={id}
            className={tab === id ? "active" : ""}
            onClick={() => {
              setTab(id);
              setActionResult("");
            }}
          >
            {label}
          </button>
        ))}
      </div>
      {error && (
        <div className="notice error" role="alert">
          {error}
        </div>
      )}
      {isDemo && (
        <div className="notice">演示模式：设置仅在当前页面内临时修改。</div>
      )}
      {tab === "general" && (
        <>
          <Panel
            title="外观与刷新"
            sub="关闭窗口后保持托盘运行，退出应用后停止桌面采集"
          >
            <div className="form-grid">
              <Field label="主题">
                <select
                  value={draft.theme}
                  onChange={(e) =>
                    setDraft({ ...draft, theme: e.target.value })
                  }
                >
                  <option value="system">跟随系统</option>
                  <option value="dark">深色</option>
                  <option value="light">浅色</option>
                </select>
              </Field>
              <Field label="用量扫描周期（秒）">
                <input
                  type="number"
                  min={10}
                  max={3600}
                  value={draft.pollSeconds}
                  onChange={(e) =>
                    setDraft({ ...draft, pollSeconds: Number(e.target.value) })
                  }
                />
              </Field>
              <Field label="额度 / 余额刷新周期（秒）">
                <input
                  type="number"
                  min={60}
                  max={3600}
                  value={draft.quotaSeconds}
                  onChange={(e) =>
                    setDraft({ ...draft, quotaSeconds: Number(e.target.value) })
                  }
                />
              </Field>
              <Field label="SSH 最大并发">
                <input
                  type="number"
                  min={1}
                  max={16}
                  value={draft.concurrency}
                  onChange={(e) =>
                    setDraft({ ...draft, concurrency: Number(e.target.value) })
                  }
                />
              </Field>
            </div>
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={draft.paused}
                onChange={(e) =>
                  setDraft({ ...draft, paused: e.target.checked })
                }
              />
              暂停桌面后台采集 <small>远端服务继续运行</small>
            </label>
          </Panel>
          <Panel
            title="数据保留"
            sub="本地统计长期保留；服务器采集器默认使用 48 小时 / 30 天 / 365 天策略"
          >
            <div className="form-grid">
              <Field label="原始资源数据（小时）">
                <input
                  type="number"
                  min={1}
                  max={720}
                  value={draft.rawHours}
                  onChange={(e) =>
                    setDraft({ ...draft, rawHours: Number(e.target.value) })
                  }
                />
              </Field>
              <Field label="分钟聚合（天）">
                <input
                  type="number"
                  min={1}
                  max={365}
                  value={draft.minuteDays}
                  onChange={(e) =>
                    setDraft({ ...draft, minuteDays: Number(e.target.value) })
                  }
                />
              </Field>
              <Field label="小时聚合（天）">
                <input
                  type="number"
                  min={30}
                  max={3650}
                  value={draft.hourDays}
                  onChange={(e) =>
                    setDraft({ ...draft, hourDays: Number(e.target.value) })
                  }
                />
              </Field>
            </div>
          </Panel>
          <div className="privacy-note">
            <ShieldCheck size={25} />
            <div>
              <strong>你的工作，留在你的设备上。</strong>
              <p>
                仅采集用量元数据；不保存提示词与回答正文。API Key 和 SSH
                密码由操作系统凭据库保管。
              </p>
            </div>
          </div>
        </>
      )}
      {tab === "sources" && (
        <Panel
          title="本地用量数据源"
          sub="Codex / Claude Code 日志，Antigravity 会话数据库"
          action={
            <button
              className="button"
              onClick={() =>
                setSource({
                  id: `source-${uid()}`,
                  agent: "codex",
                  path: "",
                  enabled: true,
                })
              }
            >
              <Plus size={15} />
              添加来源
            </button>
          }
        >
          <div className="setting-list">
            {draft.sources.map((s) => {
              const status = data.sources.find((x) => x.source === s.id);
              return (
                <div className="setting-item" key={s.id}>
                  <AgentIcon agent={s.agent} />
                  <div className="grow">
                    <strong>{agents[s.agent]?.name}</strong>
                    <p className="path">{s.path}</p>
                    <small>{status?.message ?? "等待首次扫描"}</small>
                  </div>
                  <Status value={status?.status ?? "unconfigured"} />
                  <input
                    aria-label={`启用 ${s.id}`}
                    type="checkbox"
                    checked={s.enabled}
                    onChange={(e) =>
                      setDraft({
                        ...draft,
                        sources: draft.sources.map((x) =>
                          x.id === s.id
                            ? { ...x, enabled: e.target.checked }
                            : x,
                        ),
                      })
                    }
                  />
                  <button
                    className="button small-button"
                    onClick={() => setSource({ ...s })}
                  >
                    编辑
                  </button>
                  <button
                    className="icon-button danger"
                    aria-label={`删除 ${s.id}`}
                    onClick={() =>
                      setDraft({
                        ...draft,
                        sources: draft.sources.filter((x) => x.id !== s.id),
                      })
                    }
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              );
            })}
          </div>
          <div className="panel-bottom">
            <span>移除来源不会删除已经采集的历史数据。</span>
            <button className="text-button" onClick={() => run("scan")}>
              <RefreshCw size={14} />
              立即扫描
            </button>
          </div>
        </Panel>
      )}
      {tab === "accounts" && (
        <>
          <Panel
            title="账户连接"
            sub="额度独立查询，不需要保持交互式 Agent 会话开启"
            action={
              <button
                className="button"
                onClick={() =>
                  setAccount({
                    id: `account-${uid()}`,
                    provider: "deepseek",
                    label: "DeepSeek",
                    serverId: "",
                    executable: "",
                    credentialPath: "",
                  })
                }
              >
                <Plus size={15} />
                添加账户
              </button>
            }
          >
            <div className="setting-list">
              {draft.accounts.map((a) => (
                <div className="setting-item" key={a.id}>
                  <AgentIcon agent={a.provider} />
                  <div className="grow">
                    <strong>{a.label}</strong>
                    <p>
                      {a.serverId
                        ? (draft.servers.find((s) => s.id === a.serverId)
                            ?.label ?? a.serverId)
                        : "本机"}{" "}
                      · {agents[a.provider]?.name}
                    </p>
                    <small>
                      {ago(data.quotas.find((q) => q.id === a.id)?.updatedAt)}
                    </small>
                  </div>
                  {a.provider === "deepseek" && (
                    <button
                      className="button small-button"
                      onClick={() => {
                        setKeyFor(a.id);
                        setSecret("");
                      }}
                    >
                      <KeyRound size={14} />
                      设置 Key
                    </button>
                  )}
                  <button
                    className="button small-button"
                    onClick={() => setAccount({ ...a })}
                  >
                    编辑
                  </button>
                  <button
                    className="icon-button danger"
                    aria-label={`删除账户 ${a.label}`}
                    onClick={() =>
                      setDraft({
                        ...draft,
                        accounts: draft.accounts.filter((x) => x.id !== a.id),
                      })
                    }
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              ))}
            </div>
          </Panel>
          <div className="notice">
            <ShieldCheck size={16} />
            Codex 和 Antigravity 通过各自已登录的 CLI 查询；Claude
            使用已有登录凭据查询。登录过期时需在原 Agent 重新登录。DeepSeek
            仅查询余额。
          </div>
        </>
      )}
      {tab === "servers" && (
        <>
          <Panel
            title="SSH 服务器"
            sub="支持 SSH 配置别名、私钥、ssh-agent 与跳板机"
            action={
              <button
                className="button"
                onClick={() =>
                  setServer({
                    id: `server-${uid()}`,
                    label: "",
                    host: "",
                    port: null,
                    identityFile: "",
                    jumpHost: "",
                    selectedDisks: [],
                    selectedInterfaces: [],
                    selectedGpus: [],
                    hiddenMetrics: [],
                    order: draft.servers.length,
                  })
                }
              >
                <Plus size={15} />
                添加服务器
              </button>
            }
          >
            <div className="setting-list">
              {draft.servers.map((s) => (
                <div className="setting-server" key={s.id}>
                  <div className="setting-item">
                    <span className="server-icon">
                      <Terminal size={20} />
                    </span>
                    <div className="grow">
                      <strong>{s.label}</strong>
                      <p>
                        {s.host}
                        {s.port ? `:${s.port}` : ""}
                      </p>
                    </div>
                    <button
                      className="button small-button"
                      onClick={() => setServer({ ...s })}
                    >
                      编辑
                    </button>
                    <button
                      className="icon-button danger"
                      aria-label={`删除设备 ${s.label}`}
                      onClick={() =>
                        setConfirm({
                          title: "移除服务器",
                          detail:
                            "从本机列表移除，保留已采集历史。远端服务保持运行；如需卸载，请先使用“卸载采集器”。",
                          action: async () => {
                            const next = {
                              ...draft,
                              servers: draft.servers.filter(
                                (x) => x.id !== s.id,
                              ),
                              accounts: draft.accounts.filter(
                                (a) => a.serverId !== s.id,
                              ),
                            };
                            await persist(next);
                          },
                        })
                      }
                    >
                      <Trash2 size={15} />
                    </button>
                  </div>
                  <div className="server-actions">
                    <button
                      disabled={actionBusy}
                      className="button small-button"
                      onClick={async () => {
                        const r = await action("serverKey", { id: s.id });
                        if (r?.fingerprints)
                          setConfirm({
                            title: "核对并信任主机密钥",
                            detail: `请通过可信渠道核对以下指纹：\n${r.fingerprints}`,
                            action: async () => {
                              await action("serverKey", {
                                id: s.id,
                                accept: true,
                              });
                            },
                          });
                      }}
                    >
                      <ShieldCheck size={13} />
                      主机指纹
                    </button>
                    <button
                      disabled={actionBusy}
                      className="button small-button"
                      onClick={() => action("serverProbe", { id: s.id })}
                    >
                      测试连接
                    </button>
                    <button
                      className="button small-button"
                      onClick={() => {
                        setKeyFor(`ssh-${s.id}`);
                        setSecret("");
                      }}
                    >
                      <KeyRound size={13} />
                      SSH 密码
                    </button>
                    <button
                      disabled={actionBusy}
                      className="button small-button"
                      onClick={() =>
                        setConfirm({
                          title: "安装 / 更新采集服务",
                          detail: `在 ${s.label} 的 SSH 用户目录安装 AgentDeck 采集器，启用 systemd 用户服务。不会开放公网端口。`,
                          action: async () => {
                            await action("serverInstall", {
                              id: s.id,
                              binaryPath,
                            });
                          },
                        })
                      }
                    >
                      <Download size={13} />
                      安装采集器
                    </button>
                    <button
                      disabled={actionBusy}
                      className="button small-button"
                      onClick={() => action("serverSync", { id: s.id })}
                    >
                      <RefreshCw size={13} />
                      同步
                    </button>
                    <button
                      disabled={actionBusy}
                      className="text-button danger"
                      onClick={() =>
                        setConfirm({
                          title: "卸载远端采集器",
                          detail:
                            "停止并删除采集服务，保留服务器上的历史数据库。",
                          action: async () => {
                            await action("serverUninstall", {
                              id: s.id,
                              deleteData: false,
                            });
                          },
                        })
                      }
                    >
                      卸载采集器
                    </button>
                  </div>
                </div>
              ))}
            </div>
            {!draft.servers.length && (
              <p className="table-empty">
                添加服务器并保存后，可以测试连接和安装采集器。
              </p>
            )}
          </Panel>
          <Panel
            title="采集器安装文件"
            sub="优先使用安装包内对应架构的采集器；开发环境可指定手动构建的文件"
          >
            <Field label="Linux 采集器路径（可选）">
              <input
                value={binaryPath}
                onChange={(e) => setBinaryPath(e.target.value)}
                placeholder="/path/to/agentdeck-collector"
              />
            </Field>
            <p className="small muted">
              远端需安装 systemd。若尚未启用用户常驻，安装结果会显示
              enable-linger 指引。
            </p>
          </Panel>
          {actionBusy && (
            <div className="notice">
              <Busy>正在连接服务器，请稍候…</Busy>
            </div>
          )}
          {actionResult && (
            <Panel title="操作结果">
              <pre className="result-box">{actionResult}</pre>
            </Panel>
          )}
        </>
      )}
      {tab === "prices" && (
        <>
          <Panel
            title="OpenRouter 模型定价"
            sub={`最新成功获取：${ago(data.priceUpdatedAt)} · 单价单位：美元 / 百万 Token`}
            action={
              <button className="button" onClick={() => run("refreshPrices")}>
                <RefreshCw size={15} />
                更新价格
              </button>
            }
          >
            {data.priceError && (
              <div className="notice error">{data.priceError}</div>
            )}
            <p className="muted">
              默认使用模型精确标识匹配。找不到的模型不会猜测价格；你可以添加映射或手动单价。
            </p>
            <div className="table-scroll">
              <table>
                <thead>
                  <tr>
                    <th>原始模型</th>
                    <th>OpenRouter 模型</th>
                    <th>输入 / 输出</th>
                    <th>缓存读 / 写</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {draft.priceOverrides.map((p, i) => (
                    <tr key={i}>
                      <td>{p.model}</td>
                      <td>{p.catalogId || "手动定价"}</td>
                      <td>
                        {p.input ?? "目录"} / {p.output ?? "目录"}
                      </td>
                      <td>
                        {p.cacheRead ?? "目录"} / {p.cacheWrite ?? "目录"}
                      </td>
                      <td>
                        <button
                          className="text-button"
                          onClick={() => setPrice({ ...p })}
                        >
                          编辑
                        </button>
                        <button
                          className="icon-button danger"
                          aria-label={`移除价格 ${p.model}`}
                          onClick={() =>
                            setDraft({
                              ...draft,
                              priceOverrides: draft.priceOverrides.filter(
                                (_, j) => i !== j,
                              ),
                            })
                          }
                        >
                          <Trash2 size={14} />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="panel-bottom">
              <span>修改价格会重新计算展示期内的 API 等价估算。</span>
              <button
                className="button"
                onClick={() =>
                  setPrice({
                    model: "",
                    catalogId: "",
                    input: null,
                    output: null,
                    cacheRead: null,
                    cacheWrite: null,
                  })
                }
              >
                <Plus size={15} />
                添加模型映射
              </button>
            </div>
          </Panel>
        </>
      )}
      {account && (
        <Modal title="账户设置" onClose={() => setAccount(null)}>
          <form
            onSubmit={async (e) => {
              e.preventDefault();
              try {
                await persist({
                  ...draft,
                  accounts: [
                    ...draft.accounts.filter((a) => a.id !== account.id),
                    account,
                  ],
                });
                setAccount(null);
              } catch {}
            }}
          >
            <div className="form-grid">
              <Field label="名称">
                <input
                  required
                  value={account.label}
                  onChange={(e) =>
                    setAccount({ ...account, label: e.target.value })
                  }
                />
              </Field>
              <Field label="提供方">
                <select
                  value={account.provider}
                  onChange={(e) =>
                    setAccount({
                      ...account,
                      provider: e.target.value as Account["provider"],
                      serverId:
                        e.target.value === "deepseek" ? "" : account.serverId,
                    })
                  }
                >
                  {Object.entries(agents).map(([id, a]) => (
                    <option key={id} value={id}>
                      {a.name}
                    </option>
                  ))}
                </select>
              </Field>
              <Field label="查询设备">
                <select
                  disabled={account.provider === "deepseek"}
                  value={account.serverId}
                  onChange={(e) =>
                    setAccount({ ...account, serverId: e.target.value })
                  }
                >
                  <option value="">本机</option>
                  {draft.servers.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.label}
                    </option>
                  ))}
                </select>
              </Field>
              {account.provider !== "deepseek" && (
                <Field label="CLI 可执行文件（可选）">
                  <input
                    value={account.executable}
                    onChange={(e) =>
                      setAccount({ ...account, executable: e.target.value })
                    }
                    placeholder="自动从 PATH 查找"
                  />
                </Field>
              )}
              {account.provider === "claude" && (
                <Field label="Claude 凭据文件（可选）">
                  <input
                    value={account.credentialPath}
                    onChange={(e) =>
                      setAccount({ ...account, credentialPath: e.target.value })
                    }
                    placeholder="默认读取当前用户登录凭据"
                  />
                </Field>
              )}
            </div>
            <div className="modal-actions">
              <button
                type="button"
                className="button"
                onClick={() => setAccount(null)}
              >
                取消
              </button>
              <button className="button primary" disabled={saving}>
                保存账户
              </button>
            </div>
          </form>
        </Modal>
      )}
      {server && (
        <Modal title="SSH 连接设置" onClose={() => setServer(null)}>
          <form
            onSubmit={async (e) => {
              e.preventDefault();
              try {
                await persist({
                  ...draft,
                  servers: [
                    ...draft.servers.filter((s) => s.id !== server.id),
                    server,
                  ],
                });
                setServer(null);
              } catch {}
            }}
          >
            <div className="form-grid">
              <Field label="设备名称">
                <input
                  required
                  value={server.label}
                  onChange={(e) =>
                    setServer({ ...server, label: e.target.value })
                  }
                  placeholder="例如：GPU 工作站"
                />
              </Field>
              <Field label="SSH 地址或配置别名">
                <input
                  required
                  value={server.host}
                  onChange={(e) =>
                    setServer({ ...server, host: e.target.value })
                  }
                  placeholder="user@host / ~/.ssh/config 别名"
                />
              </Field>
              <Field label="端口（可选）">
                <input
                  type="number"
                  min={1}
                  max={65535}
                  value={server.port ?? ""}
                  onChange={(e) =>
                    setServer({
                      ...server,
                      port: e.target.value ? Number(e.target.value) : null,
                    })
                  }
                  placeholder="使用 SSH 配置或 22"
                />
              </Field>
              <Field label="私钥路径（可选）">
                <input
                  value={server.identityFile}
                  onChange={(e) =>
                    setServer({ ...server, identityFile: e.target.value })
                  }
                  placeholder="使用 SSH 配置或 ssh-agent"
                />
              </Field>
              <Field label="跳板机（可选）">
                <input
                  value={server.jumpHost}
                  onChange={(e) =>
                    setServer({ ...server, jumpHost: e.target.value })
                  }
                  placeholder="user@jump-host"
                />
              </Field>
              <Field label="显示顺序">
                <input
                  type="number"
                  value={server.order}
                  onChange={(e) =>
                    setServer({ ...server, order: Number(e.target.value) })
                  }
                />
              </Field>
            </div>
            <div className="modal-actions">
              <button
                type="button"
                className="button"
                onClick={() => setServer(null)}
              >
                取消
              </button>
              <button className="button primary" disabled={saving}>
                保存连接
              </button>
            </div>
          </form>
        </Modal>
      )}
      {source && (
        <Modal title="用量数据源" onClose={() => setSource(null)}>
          <form
            onSubmit={async (e) => {
              e.preventDefault();
              try {
                await persist({
                  ...draft,
                  sources: [
                    ...draft.sources.filter((s) => s.id !== source.id),
                    source,
                  ],
                });
                setSource(null);
              } catch {}
            }}
          >
            <Field label="Agent">
              <select
                value={source.agent}
                onChange={(e) =>
                  setSource({ ...source, agent: e.target.value })
                }
              >
                {Object.entries(agents)
                  .filter(([id]) => id !== "deepseek")
                  .map(([id, a]) => (
                    <option key={id} value={id}>
                      {a.name}
                    </option>
                  ))}
              </select>
            </Field>
            <Field
              label="会话数据目录"
              hint="使用完整路径；支持递归读取子目录。"
            >
              <input
                required
                value={source.path}
                onChange={(e) => setSource({ ...source, path: e.target.value })}
              />
            </Field>
            <div className="modal-actions">
              <button className="button primary" disabled={saving}>
                <FolderOpen size={15} />
                保存来源
              </button>
            </div>
          </form>
        </Modal>
      )}
      {keyFor && (
        <Modal
          title={
            keyFor.startsWith("ssh-")
              ? "SSH 密码 / 私钥口令"
              : "DeepSeek API Key"
          }
          onClose={() => {
            setKeyFor(null);
            setSecret("");
          }}
        >
          <p className="muted">
            保存到当前操作系统的凭据库。应用不回显已保存的内容。留空保存会删除现有凭据。
          </p>
          <Field label="密钥">
            <input
              type="password"
              autoComplete="new-password"
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
              placeholder="输入新的凭据"
            />
          </Field>
          <div className="modal-actions">
            <button
              className="button primary"
              onClick={async () => {
                const r = await action("saveSecret", {
                  id: keyFor,
                  value: secret,
                });
                if (r) {
                  setKeyFor(null);
                  setSecret("");
                }
              }}
            >
              <KeyRound size={15} />
              保存到凭据库
            </button>
          </div>
        </Modal>
      )}
      {price && (
        <Modal title="模型映射与单价" onClose={() => setPrice(null)}>
          <form
            onSubmit={async (e) => {
              e.preventDefault();
              try {
                await persist({
                  ...draft,
                  priceOverrides: [
                    ...draft.priceOverrides.filter(
                      (p) => p.model !== price.model,
                    ),
                    price,
                  ],
                });
                setPrice(null);
              } catch {}
            }}
          >
            <Field label="日志中的模型名">
              <input
                required
                value={price.model}
                onChange={(e) => setPrice({ ...price, model: e.target.value })}
                list="models"
              />
              <datalist id="models">
                {[...new Set(data.usage.map((r) => r.model))].map((m) => (
                  <option key={m}>{m}</option>
                ))}
              </datalist>
            </Field>
            <Field label="OpenRouter 模型 ID（可选）">
              <input
                value={price.catalogId}
                onChange={(e) =>
                  setPrice({ ...price, catalogId: e.target.value })
                }
                placeholder="例如 anthropic/claude-sonnet-4"
              />
            </Field>
            <div className="form-grid">
              {(
                [
                  ["input", "输入"],
                  ["output", "输出"],
                  ["cacheRead", "缓存读取"],
                  ["cacheWrite", "缓存写入"],
                ] as const
              ).map(([key, label]) => (
                <Field key={key} label={`${label} · $ / 百万 Token`}>
                  <input
                    type="number"
                    min={0}
                    step="any"
                    value={price[key] ?? ""}
                    placeholder="使用目录价格"
                    onChange={(e) =>
                      setPrice({
                        ...price,
                        [key]:
                          e.target.value === "" ? null : Number(e.target.value),
                      })
                    }
                  />
                </Field>
              ))}
            </div>
            <div className="modal-actions">
              <button className="button primary" disabled={saving}>
                保存映射
              </button>
            </div>
          </form>
        </Modal>
      )}
      {confirm && (
        <Modal title={confirm.title} onClose={() => setConfirm(null)}>
          <p className="confirm-detail">{confirm.detail}</p>
          <div className="modal-actions">
            <button className="button" onClick={() => setConfirm(null)}>
              取消
            </button>
            <button
              className="button primary"
              disabled={actionBusy}
              onClick={async () => {
                await confirm.action();
                setConfirm(null);
              }}
            >
              {actionBusy ? <Busy>执行中</Busy> : "确认"}
            </button>
          </div>
        </Modal>
      )}
    </>
  );
}
