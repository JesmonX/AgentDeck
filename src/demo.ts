// Explicit ?demo=1 visual fixture. Never substituted for a failed real connection.
import type { Snapshot, Sample, Usage, Settings, History } from "./types";
const now = Math.floor(Date.now() / 1000),
  models = [
    "gpt-5.4",
    "claude-opus-4.6",
    "gemini-3.1-pro",
    "claude-sonnet-4.6",
  ];
const usage: Usage[] = [];
for (let day = 0; day < 365; day++)
  for (let m = 0; m < 4; m++) {
    const ts = now - day * 86400,
      activity = Math.sin(day * 1.7 + m) + 1.3;
    if (day % 7 === 0 && day > 7) continue;
    const input = Math.round(
        (370000 + activity * 490000) *
          (1 + (365 - day) / 190) *
          (m === 0 ? 1.5 : 0.7),
      ),
      output = Math.round(input * 0.065),
      cacheRead = Math.round(input * (0.62 + m * 0.055));
    usage.push({
      timestamp: ts,
      agent: ["codex", "claude", "agy", "claude"][m],
      model: models[m],
      device: day % 3 ? "demo-local" : "demo-gpu",
      input,
      output,
      cacheRead,
      cacheWrite: m === 1 ? Math.round(input * 0.03) : 0,
      cacheKnownInput: input,
      requests: Math.round(activity * 16),
      cost:
        (input - cacheRead) * 0.000003 +
        cacheRead * 0.0000003 +
        output * 0.000015,
      pricedTokens: input + output,
    });
  }
function sample(name: string, scale = 1): Sample {
  return {
    timestamp: now,
    hostname: name,
    bootId: "demo",
    uptime: 86400 * 24 + 38000,
    cpu: { usage: 37.4 * scale, user: 29.2 * scale, system: 6.9, ioWait: 1.3 },
    cores: Array.from({ length: 32 }, (_, i) => ({
      id: `cpu${i}`,
      usage: (i * 17 + 23) % 100,
    })),
    load: [8.24, 7.12, 6.89],
    memory: {
      total: 256 * 2 ** 30,
      used: 142 * 2 ** 30,
      available: 114 * 2 ** 30,
      cached: 32 * 2 ** 30,
      swapTotal: 16 * 2 ** 30,
      swapUsed: 1.2 * 2 ** 30,
    },
    network: [
      {
        id: "eth0",
        receivedBytes: 821 * 2 ** 30,
        sentBytes: 314 * 2 ** 30,
        rxRate: 12.4 * 2 ** 20,
        txRate: 3.8 * 2 ** 20,
        dailyReceived: 28.7 * 2 ** 30,
        dailySent: 12.3 * 2 ** 30,
        partial: false,
        defaultVisible: true,
      },
    ],
    disks: [
      {
        id: "/",
        device: "/dev/nvme0n1p2",
        total: 2 * 2 ** 40,
        used: 0.9 * 2 ** 40,
        available: 1.1 * 2 ** 40,
        usage: 45,
        inodeUsage: 12,
      },
      {
        id: "/data",
        device: "/dev/nvme1n1",
        total: 4 * 2 ** 40,
        used: 2.7 * 2 ** 40,
        available: 1.3 * 2 ** 40,
        usage: 67.5,
        inodeUsage: 23,
      },
    ],
    diskIo: [
      {
        id: "nvme0n1",
        readRate: 24 * 2 ** 20,
        writeRate: 8 * 2 ** 20,
        readIops: 960,
        writeIops: 240,
      },
    ],
    gpus: Array.from({ length: 4 }, (_, i) => ({
      id: `GPU-demo-${i}`,
      name: "NVIDIA RTX 4090",
      usage: 72 + i * 5,
      memoryUsed: (18 + i) * 2 ** 30,
      memoryTotal: 24 * 2 ** 30,
      temperature: 63 + i * 2,
      power: 285 + i * 12,
    })),
    gpuProcesses: [
      { gpu: "GPU-demo-0", pid: "18240", name: "python", memory: 18 * 2 ** 30 },
    ],
    gpuError: null,
    diskError: null,
    sampleCount: 1,
  };
}
const server = (id: string, label: string, host: string, order: number) => ({
  id,
  label,
  host,
  order,
  port: null,
  identityFile: "",
  jumpHost: "",
  selectedDisks: [],
  selectedInterfaces: [],
  selectedGpus: [],
  hiddenMetrics: [],
});
const data: Snapshot = {
  device: "demo-local",
  usage,
  settings: {
    theme: "dark",
    sources: [
      { id: "codex", agent: "codex", path: "~/.codex/sessions", enabled: true },
      {
        id: "claude",
        agent: "claude",
        path: "~/.claude/projects",
        enabled: true,
      },
      { id: "agy", agent: "agy", path: "~/.gemini", enabled: true },
    ],
    accounts: [],
    servers: [
      server("gpu", "GPU 工作站", "dev@gpu-workstation", 0),
      server("build", "构建服务器", "dev@build-server", 1),
    ],
    pollSeconds: 60,
    quotaSeconds: 300,
    concurrency: 4,
    paused: false,
    priceOverrides: [],
    rawHours: 48,
    minuteDays: 30,
    hourDays: 365,
  },
  sources: [],
  quotas: ["codex", "claude", "agy"].map((provider, i) => ({
    id: provider,
    provider,
    label: ["Codex", "Claude Code", "Antigravity"][i],
    device: "本机",
    accountId: null,
    windows: [
      {
        id: "5h",
        label: "5 小时",
        usedPercent: [32, 58, 16][i],
        resetsAt: now + [10800, 6200, 14200][i],
        durationMinutes: 300,
      },
      {
        id: "7d",
        label: "7 天",
        usedPercent: [46, 29, 38][i],
        resetsAt: now + [230000, 412000, 328000][i],
        durationMinutes: 10080,
      },
    ],
    resetCards:
      i === 0
        ? {
            availableCount: 2,
            credits: [
              {
                id: "1",
                title: "额度重置卡",
                status: "available",
                expiresAt: now + 86400 * 10,
              },
            ],
          }
        : null,
    balance: null,
    updatedAt: now - 45,
    attemptedAt: now - 45,
    status: "ready",
    error: null,
  })),
  servers: [
    {
      id: "gpu",
      device: "demo-gpu",
      label: "GPU 工作站",
      status: "ready",
      updatedAt: now,
      sample: sample("gpu-workstation"),
    },
    {
      id: "build",
      device: "demo-build",
      label: "构建服务器",
      status: "ready",
      updatedAt: now,
      sample: {
        ...sample("build-server", 0.42),
        gpus: [],
        gpuProcesses: [],
        gpuError: "未安装 NVIDIA GPU",
      },
    },
  ],
  priceUpdatedAt: now - 3600,
  priceError: null,
  generatedAt: now,
};
data.quotas.push({
  id: "deepseek",
  provider: "deepseek",
  label: "DeepSeek",
  device: "本机",
  accountId: null,
  windows: [],
  resetCards: null,
  balance: {
    is_available: true,
    balance_infos: [
      {
        currency: "CNY",
        total_balance: "128.64",
        granted_balance: "18.00",
        topped_up_balance: "110.64",
      },
    ],
  },
  updatedAt: now - 80,
  attemptedAt: now - 80,
  status: "ready",
  error: null,
});
data.settings.accounts = data.quotas.map((q) => ({
  id: q.id,
  label: q.label,
  provider: q.provider as "codex",
  serverId: "",
  executable: "",
  credentialPath: "",
}));
data.sources = data.settings.sources.map((s) => ({
  source: s.id,
  status: "ready",
  scannedAt: now,
  events: 80,
  errors: 0,
  message: "演示来源 · 非真实数据",
}));
export function demoApi(method: string, params: unknown): unknown {
  if (method === "snapshot") return structuredClone(data);
  if (method === "saveSettings") {
    data.settings = params as Settings;
    return { saved: true };
  }
  if (method === "history") {
    const p = params as { since: number };
    const rows = Array.from({ length: 120 }, (_, i) => {
      const s = sample("demo");
      s.timestamp = p.since + ((now - p.since) * i) / 119;
      s.cpu.usage = 30 + Math.sin(i * 0.2) * 18 + i / 20;
      s.memory.used = (135 + Math.sin(i * 0.07) * 8) * 2 ** 30;
      return { timestamp: s.timestamp, payload: s };
    });
    return { resolution: 60, rows } satisfies History;
  }
  if (method === "saveSecret") throw new Error("演示模式不接收或保存真实密钥");
  if (method.startsWith("server")) throw new Error("演示模式不执行 SSH 操作");
  return { demo: true };
}
