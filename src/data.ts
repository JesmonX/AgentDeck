import type { Usage, History } from "./types";
export function historySeries(
  history: History | null,
  metric: "cpu" | "memory" | "rx" | "tx",
  interfaces: string[] = [],
): [number, number | null][] {
  const points: [number, number | null][] = [];
  let previous: number | undefined;
  for (const row of history?.rows ?? []) {
    if (
      previous !== undefined &&
      row.timestamp - previous > (history?.resolution ?? 5) * 2.5
    )
      points.push([(previous + (history?.resolution ?? 5)) * 1000, null]);
    const p = row.payload;
    let value: number | null;
    if (metric === "cpu") value = p.cpu.usage;
    else if (metric === "memory")
      value = p.memory.total ? (p.memory.used / p.memory.total) * 100 : null;
    else {
      const rates = visibleNetwork(p.network, interfaces).map(
        (n) => n[metric === "rx" ? "rxRate" : "txRate"],
      );
      value =
        rates.length && rates.every((v) => typeof v === "number")
          ? rates.reduce<number>((sum, v) => sum + Number(v), 0)
          : null;
    }
    points.push([row.timestamp * 1000, value]);
    previous = row.timestamp;
  }
  return points;
}
export const agents: Record<
  string,
  { name: string; short: string; color: string }
> = {
  codex: { name: "Codex", short: "CX", color: "#86bca5" },
  claude: { name: "Claude Code", short: "CC", color: "#d89f7e" },
  agy: { name: "Antigravity", short: "AG", color: "#9f9ae0" },
  deepseek: { name: "DeepSeek", short: "DS", color: "#7daee5" },
};
export const palette = [
  "#81bba2",
  "#a99bdd",
  "#dfad86",
  "#7ba9cc",
  "#d4c078",
  "#b887a7",
  "#6fbab7",
  "#a9b7c5",
];
export const compact = (n: number | null | undefined) =>
  n == null
    ? "—"
    : Intl.NumberFormat("en", {
        notation: "compact",
        maximumFractionDigits: 1,
      }).format(n);
export const integer = (n: number) => Intl.NumberFormat("en").format(n);
export const money = (n: number) =>
  "$" +
  Intl.NumberFormat("en", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(n);
export function bytes(n: number | null | undefined) {
  if (n == null || !Number.isFinite(n)) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  while (n >= 1024 && i < 4) {
    n /= 1024;
    i++;
  }
  return `${n.toFixed(i > 1 ? 1 : 0)} ${units[i]}`;
}
export function localDay(ts: number) {
  const d = new Date(ts * 1000);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}
export function dayStart(day: string) {
  return new Date(day + "T00:00:00").getTime() / 1000;
}
export function daysBack(n: number) {
  const d = new Date();
  d.setDate(d.getDate() - n + 1);
  d.setHours(0, 0, 0, 0);
  return d.getTime() / 1000;
}
export function ago(ts: number | null | undefined) {
  if (!ts) return "尚未更新";
  const s = Math.max(0, Date.now() / 1000 - ts);
  return s < 60
    ? "刚刚更新"
    : s < 3600
      ? `${Math.floor(s / 60)} 分钟前更新`
      : `${Math.floor(s / 3600)} 小时前更新`;
}
export function countdown(ts: number | null) {
  if (!ts) return "重置时间未知";
  const s = Math.max(0, ts - Date.now() / 1000);
  if (s === 0) return "等待刷新确认";
  const h = Math.floor(s / 3600),
    m = Math.floor((s % 3600) / 60);
  return h >= 24
    ? `${Math.floor(h / 24)} 天 ${h % 24} 小时后重置`
    : `${h} 小时 ${m} 分钟后重置`;
}
export function sumUsage(rows: Usage[]) {
  return rows.reduce(
    (a, r) => ({
      input: a.input + r.input,
      output: a.output + r.output,
      cacheRead: a.cacheRead + r.cacheRead,
      cacheWrite: a.cacheWrite + r.cacheWrite,
      cacheKnownInput: a.cacheKnownInput + r.cacheKnownInput,
      cost: a.cost + r.cost,
      pricedTokens: a.pricedTokens + r.pricedTokens,
      requests: a.requests + r.requests,
    }),
    {
      input: 0,
      output: 0,
      cacheRead: 0,
      cacheWrite: 0,
      cacheKnownInput: 0,
      cost: 0,
      pricedTokens: 0,
      requests: 0,
    },
  );
}
export function byModel(rows: Usage[]) {
  const map = new Map<string, Usage[]>();
  rows.forEach((r) => map.set(r.model, [...(map.get(r.model) ?? []), r]));
  return [...map]
    .map(([model, rows]) => ({ model, ...sumUsage(rows) }))
    .sort((a, b) => b.input + b.output - (a.input + a.output));
}
export function visibleNetwork<
  T extends { id: string; defaultVisible?: string | number | boolean | null },
>(items: T[], selected: string[]) {
  return items.filter((i) =>
    selected.length ? selected.includes(i.id) : i.defaultVisible !== false,
  );
}
