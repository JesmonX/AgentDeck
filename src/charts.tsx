import { useEffect, useMemo, useRef } from "react";
import * as echarts from "echarts/core";
import { BarChart, LineChart, PieChart, HeatmapChart } from "echarts/charts";
import {
  GridComponent,
  TooltipComponent,
  LegendComponent,
  CalendarComponent,
  VisualMapComponent,
} from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import {
  palette,
  localDay,
  compact,
  byModel,
  sumUsage,
  daysBack,
  historySeries,
} from "./data";
import type { Usage, History } from "./types";
echarts.use([
  BarChart,
  LineChart,
  PieChart,
  HeatmapChart,
  GridComponent,
  TooltipComponent,
  LegendComponent,
  CalendarComponent,
  VisualMapComponent,
  CanvasRenderer,
]);
type Option = echarts.EChartsCoreOption;
export function Chart({
  option,
  height = 280,
  onSelect,
}: {
  option: Option;
  height?: number;
  onSelect?: (name: string) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    const c = echarts.init(node);
    const style = getComputedStyle(document.documentElement);
    c.setOption({
      animationDuration: 350,
      color: palette,
      textStyle: {
        fontFamily: "Inter, system-ui, sans-serif",
        color: style.getPropertyValue("--muted").trim(),
      },
      backgroundColor: "transparent",
      ...option,
    });
    const observer = new ResizeObserver(() => c.resize());
    observer.observe(node);
    if (onSelect) c.on("click", (p) => onSelect(p.name));
    return () => {
      observer.disconnect();
      c.dispose();
    };
  }, [option, onSelect]);
  return (
    <div
      role="img"
      aria-label="数据图表"
      ref={ref}
      style={{ height, width: "100%" }}
    />
  );
}
export function UsageTrend({
  rows,
  start,
  end,
  onSelect,
}: {
  rows: Usage[];
  start: number;
  end: number;
  onSelect: (s: string) => void;
}) {
  const option = useMemo(() => {
    const models = byModel(rows)
      .slice(0, 7)
      .map((m) => m.model);
    const days: string[] = [];
    const date = new Date(start * 1000);
    while (date.getTime() / 1000 <= end && days.length < 1100) {
      days.push(localDay(date.getTime() / 1000));
      date.setDate(date.getDate() + 1);
    }
    const buckets = new Map<string, Usage[]>();
    rows.forEach((r) => {
      const d = localDay(r.timestamp);
      buckets.set(d, [...(buckets.get(d) ?? []), r]);
    });
    return {
      tooltip: { trigger: "axis", confine: true },
      legend: {
        bottom: 0,
        type: "scroll",
        icon: "roundRect",
        itemWidth: 9,
        itemHeight: 9,
        textStyle: { color: "#84909d", fontSize: 11 },
      },
      grid: { left: 45, right: 42, top: 20, bottom: 65 },
      xAxis: {
        type: "category",
        data: days,
        axisLine: { show: false },
        axisTick: { show: false },
        axisLabel: { color: "#84909d", formatter: (s: string) => s.slice(5) },
      },
      yAxis: [
        {
          type: "value",
          splitLine: { lineStyle: { color: "#8e9dab18", type: "dashed" } },
          axisLabel: { color: "#84909d", formatter: compact },
        },
        {
          type: "value",
          max: 100,
          splitLine: { show: false },
          axisLabel: { color: "#84909d", formatter: "{value}%" },
        },
      ],
      series: [
        ...models.map((model, i) => ({
          name: model,
          type: "bar",
          stack: "tokens",
          barMaxWidth: 40,
          itemStyle: {
            color: palette[i % palette.length],
            borderRadius: i === models.length - 1 ? [4, 4, 0, 0] : 0,
          },
          data: days.map((d) =>
            (buckets.get(d) ?? [])
              .filter((r) => r.model === model)
              .reduce((a, r) => a + r.input + r.output, 0),
          ),
        })),
        ...(byModel(rows).length > 7
          ? [
              {
                name: "其他模型",
                type: "bar",
                stack: "tokens",
                data: days.map((d) =>
                  (buckets.get(d) ?? [])
                    .filter((r) => !models.includes(r.model))
                    .reduce((a, r) => a + r.input + r.output, 0),
                ),
              },
            ]
          : []),
        {
          name: "缓存命中率",
          type: "line",
          yAxisIndex: 1,
          smooth: true,
          symbol: "circle",
          symbolSize: 5,
          lineStyle: { color: "#d6e5e0", width: 2 },
          itemStyle: { color: "#d6e5e0" },
          data: days.map((d) => {
            const s = sumUsage(buckets.get(d) ?? []);
            return s.cacheKnownInput
              ? (s.cacheRead / s.cacheKnownInput) * 100
              : null;
          }),
        },
      ],
    };
  }, [rows, start, end]);
  return <Chart option={option} onSelect={onSelect} />;
}
export function ModelDonut({
  rows,
  mode,
}: {
  rows: Usage[];
  mode: "tokens" | "cost";
}) {
  const models = useMemo(() => byModel(rows), [rows]);
  const option = useMemo(
    () => ({
      tooltip: { trigger: "item", confine: true },
      series: [
        {
          type: "pie",
          radius: ["66%", "86%"],
          center: ["50%", "49%"],
          avoidLabelOverlap: true,
          label: { show: false },
          itemStyle: {
            borderRadius: 3,
            borderWidth: 3,
            borderColor: "transparent",
          },
          data: models.map((m) => ({
            name: m.model,
            value: mode === "cost" ? m.cost : m.input + m.output,
          })),
        },
      ],
    }),
    [models, mode],
  );
  return (
    <div className="donut-layout">
      <div className="donut-chart">
        <Chart option={option} height={210} />
        <div className="donut-center">
          <strong>{models.length}</strong>
          <span>活跃模型</span>
        </div>
      </div>
      <div className="model-legend">
        {models.slice(0, 5).map((m, i) => {
          const total = models.reduce(
            (a, m) => a + (mode === "cost" ? m.cost : m.input + m.output),
            0,
          );
          return (
            <div key={m.model}>
              <i style={{ background: palette[i % palette.length] }} />
              <span title={m.model}>{m.model}</span>
              <b>
                {total
                  ? (
                      ((mode === "cost" ? m.cost : m.input + m.output) /
                        total) *
                      100
                    ).toFixed(1)
                  : "—"}
                %
              </b>
            </div>
          );
        })}
      </div>
    </div>
  );
}
export function Heatmap({
  rows,
  days,
  onSelect,
}: {
  rows: Usage[];
  days: number;
  onSelect: (s: string) => void;
}) {
  const option = useMemo(() => {
    const totals = new Map<string, number>();
    rows.forEach((r) => {
      const d = localDay(r.timestamp);
      totals.set(d, (totals.get(d) ?? 0) + r.input + r.output);
    });
    const start = days
      ? daysBack(days)
      : Math.min(...rows.map((r) => r.timestamp), daysBack(365));
    const end = Date.now() / 1000;
    const range = [localDay(start), localDay(end)];
    const data = [...totals].filter(([d]) => d >= range[0] && d <= range[1]);
    return {
      tooltip: {
        position: "top",
        confine: true,
        formatter: (p: { value: [string, number] }) =>
          `${p.value[0]} · ${compact(p.value[1])} tokens`,
      },
      visualMap: {
        min: 0,
        max: Math.max(...data.map((d) => d[1]), 1),
        show: false,
        inRange: {
          color: ["#243c35", "#345f4b", "#538568", "#81bba2", "#b9e5ce"],
        },
      },
      calendar: {
        top: 30,
        left: 36,
        right: 12,
        bottom: 12,
        cellSize: ["auto", 15],
        range,
        splitLine: { show: false },
        itemStyle: {
          color: "#84909d0c",
          borderColor: "transparent",
          borderWidth: 3,
        },
        yearLabel: { show: false },
        dayLabel: {
          firstDay: 1,
          nameMap: ["日", "一", "二", "三", "四", "五", "六"],
          color: "#84909d",
          fontSize: 10,
        },
        monthLabel: { nameMap: "en", color: "#84909d", fontSize: 11 },
      },
      series: [{ type: "heatmap", coordinateSystem: "calendar", data }],
    };
  }, [rows, days]);
  return (
    <div className="heatmap-scroll">
      <Chart height={168} option={option} onSelect={onSelect} />
    </div>
  );
}
export function ServerTrend({
  history,
  metric,
  interfaces,
}: {
  history: History | null;
  metric: "cpu" | "memory" | "network";
  interfaces: string[];
}) {
  const option = useMemo(
    () => ({
      tooltip: { trigger: "axis", confine: true },
      legend: { bottom: 0, textStyle: { color: "#84909d" } },
      grid: { left: 50, right: 20, top: 18, bottom: 58 },
      xAxis: {
        type: "time",
        axisLine: { show: false },
        axisLabel: { color: "#84909d" },
        splitLine: { show: false },
      },
      yAxis: {
        type: "value",
        max: metric === "network" ? undefined : 100,
        axisLabel: {
          color: "#84909d",
          formatter: metric === "network" ? compact : "{value}%",
        },
        splitLine: { lineStyle: { color: "#84909d20", type: "dashed" } },
      },
      series: (metric === "network" ? ["rx", "tx"] : [metric]).map((name) => ({
        name:
          name === "rx"
            ? "下载 B/s"
            : name === "tx"
              ? "上传 B/s"
              : metric === "cpu"
                ? "CPU 使用率"
                : "内存使用率",
        type: "line",
        showSymbol: false,
        connectNulls: false,
        sampling: "lttb",
        areaStyle: { opacity: 0.06 },
        lineStyle: { width: 2 },
        data: historySeries(
          history,
          name as "cpu" | "memory" | "rx" | "tx",
          interfaces,
        ),
      })),
    }),
    [history, metric, interfaces],
  );
  return <Chart option={option} />;
}
