import { useEffect, useState } from "react";
import {
  Server as ServerIcon,
  Cpu,
  MemoryStick,
  HardDrive,
  Network,
  Plus,
  RefreshCw,
  ArrowLeft,
  SlidersHorizontal,
  Activity,
  Monitor,
  ChevronRight,
} from "lucide-react";
import type {
  Snapshot,
  ServerState,
  Server,
  History,
  DeviceMetric,
} from "./types";
import { bytes, ago, visibleNetwork } from "./data";
import { Panel, Metric, Status, Empty, Progress, Modal } from "./components";
import { ServerTrend } from "./charts";
import { api } from "./api";
const percent = (v: unknown) =>
  typeof v === "number" ? `${v.toFixed(1)}%` : "—";
const num = (v: unknown) => (typeof v === "number" ? v : null);
const allMetrics: Record<string, string> = {
  cpu: "CPU 总使用率",
  cores: "逐核使用率",
  cpuDetails: "用户 / 系统 / IO 等待",
  load: "系统负载",
  memory: "内存使用",
  cache: "内存缓存",
  swap: "Swap",
  gpu: "NVIDIA 显卡",
  gpuMemory: "GPU 显存",
  gpuTemp: "GPU 温度",
  gpuPower: "GPU 功耗",
  gpuProcesses: "GPU 进程",
  disk: "挂载点容量",
  inode: "inode",
  diskIo: "磁盘 IO",
  network: "网络速度",
  traffic: "累计流量",
};
export function Servers({
  data,
  run,
  settings,
  saveSettings,
}: {
  data: Snapshot;
  run: (method: string, params?: unknown) => Promise<unknown>;
  settings: () => void;
  saveSettings: (s: Snapshot["settings"]) => Promise<void>;
}) {
  const [selected, setSelected] = useState<string | null>(null);
  const states = data.servers;
  const state = states.find((s) => s.id === selected);
  const config = data.settings.servers.find((s) => s.id === selected);
  const fresh = (s: ServerState) =>
    s.status === "ready" &&
    !!s.sample &&
    Date.now() / 1000 - s.sample.timestamp < 30;
  if (state && config)
    return (
      <ServerDetail
        state={state}
        config={config}
        onBack={() => setSelected(null)}
        refresh={() => run("serverSync", { id: state.id })}
        save={async (s) =>
          saveSettings({
            ...data.settings,
            servers: data.settings.servers.map((x) => (x.id === s.id ? s : x)),
          })
        }
      />
    );
  const online = states.filter(fresh);
  const gpu = online.reduce((n, s) => n + (s.sample?.gpus.length ?? 0), 0);
  const memory = online.reduce((n, s) => n + (s.sample?.memory.total ?? 0), 0);
  return (
    <>
      <div className="page-heading">
        <div>
          <div className="eyebrow">YOUR INFRASTRUCTURE, CONNECTED</div>
          <h1>
            服务器
            <span className="heading-dot" />
          </h1>
          <p>从运行状态到每一块 GPU，掌握你的计算资源。</p>
        </div>
        <button className="button primary" onClick={settings}>
          <Plus size={16} />
          添加服务器
        </button>
      </div>
      <div className="metrics-grid">
        <Metric
          label="在线服务器"
          value={`${online.length} / ${data.settings.servers.length}`}
          note="最近 30 秒内收到采样"
          icon={<ServerIcon size={17} />}
          accent
        />
        <Metric
          label="NVIDIA GPU"
          value={online.length ? String(gpu) : "—"}
          note="当前在线设备的显卡数量"
          icon={<Cpu size={17} />}
        />
        <Metric
          label="总内存容量"
          value={online.length ? bytes(memory) : "—"}
          note="当前在线设备汇总"
          icon={<MemoryStick size={17} />}
        />
        <Metric
          label="连接方式"
          value="SSH"
          note="加密传输 · 远端持续采集"
          icon={<Network size={17} />}
        />
      </div>
      {states.length ? (
        <div className="server-grid">
          {[...states]
            .sort(
              (a, b) =>
                (data.settings.servers.find((s) => s.id === a.id)?.order ?? 0) -
                (data.settings.servers.find((s) => s.id === b.id)?.order ?? 0),
            )
            .map((s) => {
              const sample = s.sample;
              return (
                <button
                  className="server-card"
                  key={s.id}
                  onClick={() => setSelected(s.id)}
                >
                  <div className="server-card-head">
                    <span className="server-icon">
                      <ServerIcon size={23} />
                    </span>
                    <div>
                      <h2>{s.label}</h2>
                      <p>
                        {data.settings.servers.find((c) => c.id === s.id)?.host}
                      </p>
                    </div>
                    <Status
                      value={
                        fresh(s)
                          ? "ready"
                          : s.status === "ready"
                            ? "unavailable"
                            : s.status
                      }
                    />
                  </div>
                  {sample ? (
                    <>
                      <div className="server-summary">
                        <div>
                          <span>
                            <Cpu size={14} />
                            CPU
                          </span>
                          <b>{percent(sample.cpu.usage)}</b>
                          <Progress value={sample.cpu.usage} />
                        </div>
                        <div>
                          <span>
                            <MemoryStick size={14} />
                            内存
                          </span>
                          <b>
                            {percent(
                              sample.memory.total
                                ? (sample.memory.used / sample.memory.total) *
                                    100
                                : null,
                            )}
                          </b>
                          <Progress
                            value={
                              sample.memory.total
                                ? (sample.memory.used / sample.memory.total) *
                                  100
                                : null
                            }
                          />
                        </div>
                      </div>
                      <div className="server-meta">
                        <span>{sample.cores.length} 核</span>
                        <span>{bytes(sample.memory.total)}</span>
                        <span>{sample.gpus.length} GPU</span>
                        <span>{Math.floor(sample.uptime / 86400)} 天运行</span>
                      </div>
                    </>
                  ) : (
                    <div className="server-wait">
                      {s.error ?? "等待安装采集器并完成首次同步"}
                    </div>
                  )}
                  <div className="server-card-foot">
                    <span>{ago(s.updatedAt)}</span>
                    <span>
                      查看详情
                      <ChevronRight size={14} />
                    </span>
                  </div>
                </button>
              );
            })}
        </div>
      ) : (
        <Panel>
          <Empty
            title="连接你的第一台服务器"
            detail="通过 SSH 部署轻量采集器。即使电脑离线，服务器上的历史数据也会继续记录。"
            action={
              <button className="button primary" onClick={settings}>
                <Plus size={15} />
                添加 SSH 服务器
              </button>
            }
          />
        </Panel>
      )}
      <div className="architecture-strip">
        <span>
          <Monitor size={19} />
          AgentDeck 桌面端
        </span>
        <i />
        <span>
          <Network size={19} />
          SSH 加密同步
        </span>
        <i />
        <span>
          <Activity size={19} />
          Linux 持续采集
        </span>
        <p>不需要开放额外公网端口</p>
      </div>
    </>
  );
}
function ServerDetail({
  state,
  config,
  onBack,
  refresh,
  save,
}: {
  state: ServerState;
  config: Server;
  onBack: () => void;
  refresh: () => Promise<unknown>;
  save: (c: Server) => Promise<void>;
}) {
  const [hours, setHours] = useState(1),
    [metric, setMetric] = useState<"cpu" | "memory" | "network">("cpu"),
    [history, setHistory] = useState<History | null>(null),
    [error, setError] = useState(""),
    [customize, setCustomize] = useState(false),
    [draft, setDraft] = useState(config);
  useEffect(() => {
    let alive = true;
    const load = () => {
      if (state.device)
        api<History>("history", {
          device: state.device,
          since: Math.floor(Date.now() / 1000 - hours * 3600),
        })
          .then((h) => {
            if (alive) {
              setHistory(h);
              setError("");
            }
          })
          .catch((e) => {
            if (alive) setError(String(e));
          });
    };
    load();
    const id = setInterval(load, 15000);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [state.device, hours]);
  const s = state.sample,
    show = (id: string) => !config.hiddenMetrics.includes(id);
  const chosen = (items: DeviceMetric[], ids: string[]) =>
    items.filter((i) => !ids.length || ids.includes(i.id));
  return (
    <>
      <button className="text-button back-button" onClick={onBack}>
        <ArrowLeft size={15} />
        所有服务器
      </button>
      <div className="page-heading">
        <div>
          <div className="eyebrow">SERVER DETAILS</div>
          <h1>{state.label}</h1>
          <p>
            {config.host} <span className="separator">/</span>{" "}
            {s?.hostname ?? "等待采集"} <span className="separator">/</span>{" "}
            {ago(state.updatedAt)}
          </p>
        </div>
        <div className="inline">
          <button
            className="button"
            onClick={() => {
              setDraft(config);
              setCustomize(true);
            }}
          >
            <SlidersHorizontal size={15} />
            自定义指标
          </button>
          <button className="button" onClick={refresh}>
            <RefreshCw size={15} />
            同步
          </button>
        </div>
      </div>
      {(state.error || error) && (
        <div className="notice error">{state.error || error}</div>
      )}
      {!s ? (
        <Panel>
          <Empty
            title="尚未收到资源采样"
            detail="在设置中检查 SSH 连接和采集服务状态，再同步设备。"
          />
        </Panel>
      ) : (
        <>
          <div className="metrics-grid">
            {show("cpu") && (
              <Metric
                label="CPU 使用率"
                value={percent(s.cpu.usage)}
                note={`${s.cores.length} 个逻辑核心`}
                icon={<Cpu size={17} />}
                accent
              />
            )}
            {show("memory") && (
              <Metric
                label="内存已用"
                value={bytes(s.memory.used)}
                note={`总容量 ${bytes(s.memory.total)}`}
                icon={<MemoryStick size={17} />}
              />
            )}
            <Metric
              label="GPU 设备"
              value={String(s.gpus.length)}
              note={s.gpuError ?? "NVIDIA 驱动已连接"}
              icon={<Cpu size={17} />}
            />
            <Metric
              label="运行时间"
              value={`${Math.floor(s.uptime / 86400)} 天`}
              note={`${Math.floor((s.uptime % 86400) / 3600)} 小时 · ${s.hostname}`}
              icon={<Activity size={17} />}
            />
          </div>
          <Panel
            title="资源趋势"
            sub="断线后的历史在重连时补齐；空白表示未采集"
            action={
              <div className="inline">
                <select
                  aria-label="趋势指标"
                  value={metric}
                  onChange={(e) => setMetric(e.target.value as typeof metric)}
                >
                  <option value="cpu">CPU</option>
                  <option value="memory">内存</option>
                  <option value="network">网络</option>
                </select>
                <select
                  aria-label="资源趋势时间"
                  value={hours}
                  onChange={(e) => setHours(Number(e.target.value))}
                >
                  <option value={1}>1 小时</option>
                  <option value={24}>24 小时</option>
                  <option value={168}>7 天</option>
                  <option value={720}>30 天</option>
                </select>
              </div>
            }
          >
            {history?.rows.length ? (
              <ServerTrend
                history={history}
                metric={metric}
                interfaces={config.selectedInterfaces}
              />
            ) : (
              <Empty
                title="历史正在积累"
                detail="首次接入后，趋势图会随着采样逐渐生成。"
              />
            )}
          </Panel>
          <div className="detail-grid">
            {(show("cores") || show("cpuDetails") || show("load")) && (
              <Panel title="处理器" action={<Cpu size={17} />}>
                {show("cpuDetails") && (
                  <div className="detail-row">
                    <span>用户 {percent(s.cpu.user)}</span>
                    <span>系统 {percent(s.cpu.system)}</span>
                    <span>IO 等待 {percent(s.cpu.ioWait)}</span>
                  </div>
                )}
                {show("load") && (
                  <div className="detail-row">
                    <span>1 / 5 / 15 分钟负载</span>
                    <b>{s.load.map((n) => n.toFixed(2)).join(" / ")}</b>
                  </div>
                )}
                {show("cores") && (
                  <div className="core-grid">
                    {s.cores.map((c) => (
                      <div key={c.id} title={`${c.id} ${percent(c.usage)}`}>
                        <span>{c.id}</span>
                        <Progress value={num(c.usage)} />
                        <b>{percent(c.usage)}</b>
                      </div>
                    ))}
                  </div>
                )}
              </Panel>
            )}
            {(show("memory") || show("cache") || show("swap")) && (
              <Panel title="内存" action={<MemoryStick size={17} />}>
                {show("memory") && (
                  <>
                    <div className="detail-row">
                      <span>已用 / 总量</span>
                      <b>
                        {bytes(s.memory.used)} / {bytes(s.memory.total)}
                      </b>
                    </div>
                    <Progress value={(s.memory.used / s.memory.total) * 100} />
                    <div className="detail-row">
                      <span>可用</span>
                      <b>{bytes(s.memory.available)}</b>
                    </div>
                  </>
                )}
                {show("cache") && (
                  <div className="detail-row">
                    <span>文件缓存</span>
                    <b>{bytes(s.memory.cached)}</b>
                  </div>
                )}
                {show("swap") && (
                  <div className="detail-row">
                    <span>Swap 已用 / 总量</span>
                    <b>
                      {bytes(s.memory.swapUsed)} / {bytes(s.memory.swapTotal)}
                    </b>
                  </div>
                )}
              </Panel>
            )}
          </div>
          {show("gpu") && (
            <Panel title="NVIDIA GPU" sub="各显卡独立显示；不支持的字段显示 —">
              {s.gpus.length ? (
                <div className="gpu-grid">
                  {chosen(s.gpus, config.selectedGpus).map((g) => (
                    <div className="gpu-card" key={g.id}>
                      <div className="detail-row">
                        <b>{String(g.name)}</b>
                        <span className="positive">{percent(g.usage)}</span>
                      </div>
                      <Progress value={num(g.usage)} />
                      {show("gpuMemory") && (
                        <div className="detail-row">
                          <span>显存</span>
                          <b>
                            {bytes(num(g.memoryUsed))} /{" "}
                            {bytes(num(g.memoryTotal))}{" "}
                            <small>
                              (
                              {percent(
                                (Number(g.memoryUsed) / Number(g.memoryTotal)) *
                                  100,
                              )}
                              )
                            </small>
                          </b>
                        </div>
                      )}
                      <div className="gpu-bottom">
                        {show("gpuTemp") && (
                          <span>温度 {g.temperature ?? "—"} °C</span>
                        )}
                        {show("gpuPower") && (
                          <span>功耗 {g.power ?? "—"} W</span>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="muted">{s.gpuError ?? "没有检测到显卡"}</p>
              )}
              {show("gpuProcesses") && s.gpuProcesses.length > 0 && (
                <div className="table-scroll">
                  <table>
                    <thead>
                      <tr>
                        <th>进程</th>
                        <th>PID</th>
                        <th>GPU</th>
                        <th>显存</th>
                      </tr>
                    </thead>
                    <tbody>
                      {s.gpuProcesses
                        .filter(
                          (p) =>
                            !config.selectedGpus.length ||
                            config.selectedGpus.includes(p.gpu),
                        )
                        .map((p, i) => (
                          <tr key={i}>
                            <td>{p.name}</td>
                            <td>{p.pid}</td>
                            <td title={p.gpu}>{p.gpu.slice(-12)}</td>
                            <td>{bytes(p.memory)}</td>
                          </tr>
                        ))}
                    </tbody>
                  </table>
                </div>
              )}
            </Panel>
          )}
          {show("disk") && (
            <Panel title="存储空间" action={<HardDrive size={17} />}>
              <div className="table-scroll">
                <table>
                  <thead>
                    <tr>
                      <th>挂载点 / 设备</th>
                      <th>已用 / 总容量</th>
                      <th>使用率</th>
                      {show("inode") && <th>inode</th>}
                    </tr>
                  </thead>
                  <tbody>
                    {chosen(s.disks, config.selectedDisks).map((d) => (
                      <tr key={d.id}>
                        <td>
                          <b>{d.id}</b>
                          <small className="block">{d.device}</small>
                        </td>
                        <td>
                          {bytes(num(d.used))} / {bytes(num(d.total))}
                        </td>
                        <td>
                          <div className="inline">
                            <Progress value={num(d.usage)} />
                            {percent(d.usage)}
                          </div>
                        </td>
                        {show("inode") && <td>{percent(d.inodeUsage)}</td>}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              {s.diskError && <p className="muted">{s.diskError}</p>}
            </Panel>
          )}
          {show("diskIo") && (
            <Panel title="块设备 IO">
              <div className="table-scroll">
                <table>
                  <thead>
                    <tr>
                      <th>设备</th>
                      <th>读取</th>
                      <th>写入</th>
                      <th>读 IOPS</th>
                      <th>写 IOPS</th>
                    </tr>
                  </thead>
                  <tbody>
                    {s.diskIo.map((d) => (
                      <tr key={d.id}>
                        <td>{d.id}</td>
                        <td>{bytes(num(d.readRate))}/s</td>
                        <td>{bytes(num(d.writeRate))}/s</td>
                        <td>{num(d.readIops)?.toFixed(0) ?? "—"}</td>
                        <td>{num(d.writeIops)?.toFixed(0) ?? "—"}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </Panel>
          )}
          {show("network") && (
            <Panel
              title="网络接口"
              sub="每日流量按 UTC 归档 · 首日或缺口标为部分记录"
              action={<Network size={17} />}
            >
              <div className="table-scroll">
                <table>
                  <thead>
                    <tr>
                      <th>接口</th>
                      <th>↓ 下载</th>
                      <th>↑ 上传</th>
                      {show("traffic") && (
                        <>
                          <th>今日下载 / 上传</th>
                          <th>系统累计收 / 发</th>
                        </>
                      )}
                    </tr>
                  </thead>
                  <tbody>
                    {visibleNetwork(s.network, config.selectedInterfaces).map(
                      (n) => (
                        <tr key={n.id}>
                          <td>{n.id}</td>
                          <td className="positive">{bytes(num(n.rxRate))}/s</td>
                          <td>{bytes(num(n.txRate))}/s</td>
                          {show("traffic") && (
                            <>
                              <td>
                                {bytes(num(n.dailyReceived))} /{" "}
                                {bytes(num(n.dailySent))}
                                {n.partial && (
                                  <small className="block">部分记录</small>
                                )}
                              </td>
                              <td>
                                {bytes(num(n.receivedBytes))} /{" "}
                                {bytes(num(n.sentBytes))}
                              </td>
                            </>
                          )}
                        </tr>
                      ),
                    )}
                  </tbody>
                </table>
              </div>
            </Panel>
          )}
          {show("traffic") && state.device && (
            <TrafficHistory
              device={state.device}
              interfaces={config.selectedInterfaces}
            />
          )}
        </>
      )}
      {customize && (
        <Modal title="自定义显示项目" onClose={() => setCustomize(false)} wide>
          <p className="muted">
            设置仅作用于这台服务器，隐藏的指标仍会持续采集。
          </p>
          <div className="checkbox-grid">
            {Object.entries(allMetrics).map(([key, label]) => (
              <label key={key}>
                <input
                  type="checkbox"
                  checked={!draft.hiddenMetrics.includes(key)}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      hiddenMetrics: e.target.checked
                        ? draft.hiddenMetrics.filter((x) => x !== key)
                        : [...draft.hiddenMetrics, key],
                    })
                  }
                />
                {label}
              </label>
            ))}
          </div>
          {s &&
            (
              [
                ["selectedDisks", "挂载点", s.disks],
                ["selectedInterfaces", "网卡", s.network],
                ["selectedGpus", "显卡", s.gpus],
              ] as const
            ).map(([key, label, items]) => (
              <div key={key}>
                <h3>
                  {label}
                  <small> · 不选择时使用默认项目</small>
                </h3>
                <div className="checkbox-grid">
                  {items.map((item) => (
                    <label key={item.id}>
                      <input
                        type="checkbox"
                        checked={draft[key].includes(item.id)}
                        onChange={(e) =>
                          setDraft({
                            ...draft,
                            [key]: e.target.checked
                              ? [...draft[key], item.id]
                              : draft[key].filter((x) => x !== item.id),
                          })
                        }
                      />
                      {item.id}
                    </label>
                  ))}
                </div>
              </div>
            ))}
          <div className="modal-actions">
            <button className="button" onClick={() => setCustomize(false)}>
              取消
            </button>
            <button
              className="button primary"
              onClick={async () => {
                await save(draft);
                setCustomize(false);
              }}
            >
              保存显示设置
            </button>
          </div>
        </Modal>
      )}
    </>
  );
}
type TrafficRow = {
  date: string;
  id: string;
  received: number | null;
  sent: number | null;
  partial: boolean;
  defaultVisible: boolean;
};
function TrafficHistory({
  device,
  interfaces,
}: {
  device: string;
  interfaces: string[];
}) {
  const [rows, setRows] = useState<TrafficRow[]>([]),
    [group, setGroup] = useState("day"),
    [error, setError] = useState("");
  useEffect(() => {
    let live = true;
    api<TrafficRow[]>("trafficHistory", {
      device,
      since: Math.floor(Date.now() / 1000 - 365 * 86400),
    })
      .then((r) => {
        if (live) setRows(Array.isArray(r) ? r : []);
      })
      .catch((e) => {
        if (live) setError(String(e));
      });
    return () => {
      live = false;
    };
  }, [device]);
  const buckets = new Map<
    string,
    { received: number; sent: number; partial: boolean; days: Set<string> }
  >();
  for (const row of visibleNetwork(rows, interfaces)) {
    const key = group === "day" ? row.date : row.date.slice(0, 7);
    const b = buckets.get(key) ?? {
      received: 0,
      sent: 0,
      partial: false,
      days: new Set<string>(),
    };
    b.received += row.received ?? 0;
    b.sent += row.sent ?? 0;
    b.partial ||= row.partial || row.received == null || row.sent == null;
    b.days.add(row.date);
    buckets.set(key, b);
  }
  return (
    <Panel
      title="流量历史"
      sub="按所选网卡汇总；月度合计仅覆盖已采集日期"
      action={
        <div className="segmented">
          <button
            className={group === "day" ? "active" : ""}
            onClick={() => setGroup("day")}
          >
            每日
          </button>
          <button
            className={group === "month" ? "active" : ""}
            onClick={() => setGroup("month")}
          >
            每月
          </button>
        </div>
      }
    >
      {error && <p className="muted">{error}</p>}
      {buckets.size ? (
        <div className="table-scroll traffic-table">
          <table>
            <thead>
              <tr>
                <th>日期（UTC）</th>
                <th>下载</th>
                <th>上传</th>
                <th>总量</th>
                <th>覆盖</th>
              </tr>
            </thead>
            <tbody>
              {[...buckets]
                .sort(([a], [b]) => b.localeCompare(a))
                .map(([date, b]) => (
                  <tr key={date}>
                    <td>{date}</td>
                    <td>{bytes(b.received)}</td>
                    <td>{bytes(b.sent)}</td>
                    <td>{bytes(b.received + b.sent)}</td>
                    <td>
                      {b.days.size} 天{b.partial ? " · 部分记录" : ""}
                    </td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
      ) : (
        <Empty
          title="流量历史正在积累"
          detail="采集器会保存每天的累计收发流量，不推算安装前的数据。"
        />
      )}
    </Panel>
  );
}
