import { useMemo, useState, useCallback } from "react";
import {
  Activity,
  Layers,
  Database,
  DollarSign,
  RefreshCw,
  ArrowUpRight,
  Ticket,
  Clock3,
  Info,
} from "lucide-react";
import type { Snapshot, Quota } from "./types";
import {
  agents,
  compact,
  money,
  integer,
  sumUsage,
  byModel,
  daysBack,
  dayStart,
  localDay,
  ago,
  countdown,
  palette,
} from "./data";
import {
  Panel,
  Metric,
  Empty,
  AgentIcon,
  Progress,
  Modal,
  Busy,
} from "./components";
import { UsageTrend, ModelDonut, Heatmap } from "./charts";

export function Agents({
  data,
  run,
  openSettings,
}: {
  data: Snapshot;
  run: (method: string, params?: unknown) => Promise<unknown>;
  openSettings: () => void;
}) {
  const [agent, setAgent] = useState("all"),
    [device, setDevice] = useState("all"),
    [model, setModel] = useState("all"),
    [range, setRange] = useState("7"),
    [customStart, setCustomStart] = useState(localDay(daysBack(7))),
    [customEnd, setCustomEnd] = useState(localDay(Date.now() / 1000)),
    [heatDays, setHeatDays] = useState(365),
    [pieMode, setPieMode] = useState<"tokens" | "cost">("tokens"),
    [selectedDay, setSelectedDay] = useState<string | null>(null);
  const allRows = useMemo(
    () =>
      data.usage.filter(
        (r) =>
          (agent === "all" || r.agent === agent) &&
          (device === "all" || r.device === device) &&
          (model === "all" || r.model === model),
      ),
    [data.usage, agent, device, model],
  );
  const start =
    range === "custom"
      ? dayStart(customStart)
      : range === "all"
        ? Math.min(...allRows.map((r) => r.timestamp), daysBack(7))
        : daysBack(Number(range));
  const end =
    range === "custom"
      ? new Date(customEnd + "T23:59:59").getTime() / 1000
      : Date.now() / 1000;
  const rows = useMemo(
    () => allRows.filter((r) => r.timestamp >= start && r.timestamp <= end),
    [allRows, start, end],
  );
  const totals = sumUsage(rows),
    total = totals.input + totals.output;
  const cacheRate = totals.cacheKnownInput
    ? (totals.cacheRead / totals.cacheKnownInput) * 100
    : null;
  const devices = [...new Set(data.usage.map((r) => r.device))];
  const models = [...new Set(data.usage.map((r) => r.model))].sort();
  const deviceName = (id: string) =>
    id === data.device
      ? "本机"
      : (data.servers.find((s) => s.device === id)?.label ?? id.slice(0, 8));
  const selectDay = useCallback((day: string) => {
    if (/^\d{4}-\d{2}-\d{2}$/.test(day)) setSelectedDay(day);
  }, []);
  const qs = useMemo(() => {
    const map = new Map<string, Quota>();
    for (const q of data.quotas) {
      const key = q.accountId ? `${q.provider}:${q.accountId}` : q.id;
      const old = map.get(key);
      if (!old || (q.updatedAt ?? 0) > (old.updatedAt ?? 0)) map.set(key, q);
    }
    return [...map.values()];
  }, [data.quotas]);
  return (
    <>
      <div className="page-heading">
        <div>
          <div className="eyebrow">YOUR WORK, IN PERSPECTIVE</div>
          <h1>
            Agent 概览
            <span className="heading-dot" />
          </h1>
          <p>用量、额度与成本，一目了然。</p>
        </div>
        <button className="button" onClick={() => run("scan")}>
          <RefreshCw size={15} />
          同步用量
        </button>
      </div>
      <div className="filter-bar">
        <div className="filters">
          <select
            aria-label="Agent 筛选"
            value={agent}
            onChange={(e) => setAgent(e.target.value)}
          >
            <option value="all">所有 Agent</option>
            {Object.entries(agents)
              .filter(([id]) => id !== "deepseek")
              .map(([id, a]) => (
                <option key={id} value={id}>
                  {a.name}
                </option>
              ))}
          </select>
          <select
            aria-label="设备筛选"
            value={device}
            onChange={(e) => setDevice(e.target.value)}
          >
            <option value="all">所有设备</option>
            {devices.map((id) => (
              <option value={id} key={id}>
                {deviceName(id)}
              </option>
            ))}
          </select>
          <select
            aria-label="模型筛选"
            value={model}
            onChange={(e) => setModel(e.target.value)}
          >
            <option value="all">所有模型</option>
            {models.map((m) => (
              <option key={m}>{m}</option>
            ))}
          </select>
        </div>
        <select
          aria-label="统计时间范围"
          value={range}
          onChange={(e) => setRange(e.target.value)}
        >
          <option value="7">最近 7 天</option>
          <option value="30">最近 30 天</option>
          <option value="90">最近 90 天</option>
          <option value="365">最近 365 天</option>
          <option value="all">全部历史</option>
          <option value="custom">自定义日期</option>
        </select>
      </div>
      {range === "custom" && (
        <div className="date-range">
          <input
            aria-label="开始日期"
            type="date"
            value={customStart}
            max={customEnd}
            onChange={(e) => setCustomStart(e.target.value)}
          />
          <span>至</span>
          <input
            aria-label="结束日期"
            type="date"
            value={customEnd}
            min={customStart}
            onChange={(e) => setCustomEnd(e.target.value)}
          />
        </div>
      )}
      <div className="metrics-grid">
        <Metric
          label="总 Token 用量"
          value={rows.length ? compact(total) : "—"}
          note={
            <>
              <span className="positive">{integer(totals.requests)}</span>{" "}
              条已采集请求记录
            </>
          }
          icon={<Activity size={17} />}
          accent
        />
        <Metric
          label="输入 / 输出"
          value={
            rows.length
              ? `${compact(totals.input)} / ${compact(totals.output)}`
              : "—"
          }
          note="输入包含缓存读取与写入"
          icon={<Layers size={17} />}
        />
        <Metric
          label="缓存命中率"
          value={cacheRate == null ? "—" : `${cacheRate.toFixed(1)}%`}
          note={
            totals.input
              ? `缓存字段覆盖 ${((totals.cacheKnownInput / totals.input) * 100).toFixed(0)}% 输入`
              : "等待用量数据"
          }
          icon={<Database size={17} />}
        />
        <Metric
          label="API 等价估算"
          value={totals.pricedTokens ? money(totals.cost) : "未估价"}
          note={
            total
              ? `OpenRouter · 已定价 ${((totals.pricedTokens / total) * 100).toFixed(0)}% Token`
              : "按模型 API 单价折算"
          }
          icon={<DollarSign size={17} />}
        />
      </div>
      {data.sources.some((s) => s.status === "partial") && (
        <div className="notice">
          <Info size={15} />
          部分来源包含无法解析的记录；当前用量为已识别部分。可在设置中查看来源状态。
        </div>
      )}
      <div className="chart-grid">
        <Panel
          title="Token 用量趋势"
          sub="按模型分布 · 缓存率叠加"
          action={
            <span className="small muted">
              {localDay(start)} — {localDay(end)}
            </span>
          }
        >
          {rows.length ? (
            <UsageTrend
              rows={rows}
              start={start}
              end={end}
              onSelect={selectDay}
            />
          ) : (
            <Empty
              title="这一段时间，还没有用量记录"
              detail="添加本地数据源或连接 SSH 服务器，历史记录会自动汇总。"
              action={
                <button className="button" onClick={openSettings}>
                  管理数据源
                  <ArrowUpRight size={14} />
                </button>
              }
            />
          )}
        </Panel>
        <Panel
          title="模型分布"
          action={
            <div className="segmented">
              <button
                className={pieMode === "tokens" ? "active" : ""}
                onClick={() => setPieMode("tokens")}
              >
                Token
              </button>
              <button
                className={pieMode === "cost" ? "active" : ""}
                onClick={() => setPieMode("cost")}
              >
                费用
              </button>
            </div>
          }
        >
          {rows.length ? (
            <ModelDonut rows={rows} mode={pieMode} />
          ) : (
            <Empty title="等待模型数据" detail="模型占比将在首次采集后展示。" />
          )}
        </Panel>
      </div>
      <div className="section-heading">
        <div>
          <h2>账户额度</h2>
          <span>账户级快照 · 独立于上方用量筛选</span>
        </div>
        <button className="text-button" onClick={() => run("refreshQuota")}>
          <RefreshCw size={14} />
          刷新额度
        </button>
      </div>
      <div className="quota-grid">
        {["codex", "claude", "agy", "deepseek"].flatMap((provider) => {
          const list = qs.filter((q) => q.provider === provider);
          return list.length
            ? list.map((q) => (
                <QuotaCard
                  key={q.id}
                  q={q}
                  refresh={() => run("refreshQuota", { id: q.id })}
                />
              ))
            : [
                <div className="quota-card empty-quota" key={provider}>
                  <div className="quota-title">
                    <AgentIcon agent={provider} />
                    <h3>{agents[provider].name}</h3>
                  </div>
                  <p>
                    {provider === "deepseek"
                      ? "连接 API 账户，随时掌握余额"
                      : "尚未获取额度信息"}
                  </p>
                  <button className="text-button" onClick={openSettings}>
                    配置账户
                    <ArrowUpRight size={14} />
                  </button>
                </div>,
              ];
        })}
      </div>
      <Panel
        title="活跃足迹"
        sub="每一个色块，记录一天的 Token 用量"
        className="heatmap-panel"
        action={
          <select
            aria-label="热力图范围"
            value={heatDays}
            onChange={(e) => setHeatDays(Number(e.target.value))}
          >
            <option value={30}>30 天</option>
            <option value={90}>90 天</option>
            <option value={365}>365 天</option>
            <option value={0}>全部</option>
          </select>
        }
      >
        <Heatmap rows={allRows} days={heatDays} onSelect={selectDay} />
        <div className="heatmap-footer">
          <span>
            点击日期查看模型明细 · 本地时区{" "}
            {Intl.DateTimeFormat().resolvedOptions().timeZone}
          </span>
          <div>
            少
            {["#243c35", "#345f4b", "#538568", "#81bba2", "#b9e5ce"].map(
              (c) => (
                <i key={c} style={{ background: c }} />
              ),
            )}
            多
          </div>
        </div>
      </Panel>
      <Panel
        title="模型用量明细"
        action={
          <span className="small muted">
            价格更新：{ago(data.priceUpdatedAt)}
          </span>
        }
      >
        <div className="table-scroll">
          <table>
            <thead>
              <tr>
                <th>模型</th>
                <th>输入</th>
                <th>输出</th>
                <th>缓存读取 / 写入</th>
                <th>总 Token</th>
                <th>估算费用</th>
              </tr>
            </thead>
            <tbody>
              {byModel(rows).map((m, i) => (
                <tr key={m.model}>
                  <td>
                    <span
                      className="legend-dot"
                      style={{ background: palette[i % palette.length] }}
                    />
                    {m.model}
                  </td>
                  <td>{compact(m.input)}</td>
                  <td>{compact(m.output)}</td>
                  <td>
                    {compact(m.cacheRead)} / {compact(m.cacheWrite)}
                  </td>
                  <td>{compact(m.input + m.output)}</td>
                  <td>
                    {m.pricedTokens ? money(m.cost) : "未估价"}
                    {m.pricedTokens > 0 &&
                      m.pricedTokens < m.input + m.output && (
                        <small> 部分</small>
                      )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {!rows.length && <p className="table-empty">当前筛选下没有数据</p>}
      </Panel>
      <p className="footnote">
        <Info size={13} />
        费用表示按当前 API
        单价折算的价值，与订阅费用、余额扣款和账户限额分别计算。
      </p>
      {selectedDay && (
        <Modal
          title={`${selectedDay} · 用量明细`}
          onClose={() => setSelectedDay(null)}
        >
          <div className="day-details">
            {byModel(
              allRows.filter((r) => localDay(r.timestamp) === selectedDay),
            ).map((m) => (
              <div className="detail-row" key={m.model}>
                <span>{m.model}</span>
                <strong>
                  {integer(m.input + m.output)} <small>tokens</small>
                </strong>
              </div>
            ))}
            <p className="muted small">
              当前设备与 Agent 筛选同样适用。该日期无记录时不推断为零使用。
            </p>
          </div>
        </Modal>
      )}
    </>
  );
}
function QuotaCard({
  q,
  refresh,
}: {
  q: Quota;
  refresh: () => Promise<unknown>;
}) {
  const [busy, setBusy] = useState(false);
  const stale = !q.updatedAt || Date.now() / 1000 - q.updatedAt > 600;
  return (
    <div className="quota-card">
      <div className="quota-title">
        <AgentIcon agent={q.provider} />
        <div>
          <h3>{q.label || agents[q.provider]?.name}</h3>
          <span>{q.accountId ? "已识别账户" : q.device || "本机"}</span>
        </div>
        <button
          className="icon-button"
          disabled={busy}
          aria-label={`刷新 ${q.label}`}
          onClick={async () => {
            setBusy(true);
            try {
              await refresh();
            } finally {
              setBusy(false);
            }
          }}
        >
          <RefreshCw size={14} className={busy ? "spin" : ""} />
        </button>
      </div>
      {q.balance
        ? q.balance.balance_infos.map((b, i) => (
            <div className="balance" key={i}>
              <span>可用余额 · {b.currency}</span>
              <strong>
                {b.currency === "CNY" ? "¥" : "$"}
                {Number(b.total_balance).toFixed(2)}
              </strong>
              <div>
                <span>赠送 {b.granted_balance}</span>
                <span>充值 {b.topped_up_balance}</span>
              </div>
            </div>
          ))
        : q.windows.map((w) => (
            <div className="quota-window" key={w.id}>
              <div>
                <span title={w.label}>
                  {w.durationMinutes === 300
                    ? "5 小时"
                    : w.durationMinutes === 10080
                      ? "7 天"
                      : w.label}
                  {q.windows.length > 2 ? <small> · {w.label}</small> : null}
                </span>
                <strong>
                  {Math.max(0, 100 - w.usedPercent).toFixed(0)}
                  <small>% 剩余</small>
                </strong>
              </div>
              <Progress
                value={100 - w.usedPercent}
                color={
                  w.usedPercent > 90 ? "var(--red)" : agents[q.provider]?.color
                }
              />
              <p
                title={
                  w.resetsAt
                    ? new Date(w.resetsAt * 1000).toLocaleString()
                    : undefined
                }
              >
                <Clock3 size={11} />
                {countdown(w.resetsAt)}
              </p>
            </div>
          ))}
      {q.resetCards && (
        <div className="reset-cards">
          <Ticket size={14} />
          <span>{q.resetCards.availableCount} 张重置卡</span>
          {!!q.resetCards.credits?.length && (
            <details>
              <summary>查看详情</summary>
              {q.resetCards.credits.map((c) => (
                <p key={c.id}>
                  {c.title || "重置卡"} · {c.status}
                  <br />
                  {c.expiresAt
                    ? `到期：${new Date(c.expiresAt * 1000).toLocaleString()}`
                    : "未提供到期时间"}
                </p>
              ))}
            </details>
          )}
        </div>
      )}
      {!q.windows.length && !q.balance && !q.resetCards && (
        <p className="quota-unavailable">
          {busy ? <Busy>正在查询</Busy> : "额度暂不可用"}
        </p>
      )}
      {q.error && (
        <div className="quota-error" title={q.error}>
          {q.error}
        </div>
      )}
      <div className="quota-updated">
        <i className={q.status === "ready" && !stale ? "live" : "stale"} />
        {ago(q.updatedAt)}
        {stale && q.updatedAt ? " · 旧快照" : ""}
      </div>
    </div>
  );
}
