export type Agent = "codex" | "claude" | "agy" | "deepseek";
export interface Source {
  id: string;
  agent: string;
  path: string;
  enabled: boolean;
}
export interface Account {
  id: string;
  provider: Agent;
  label: string;
  serverId: string;
  executable: string;
  credentialPath: string;
}
export interface Server {
  id: string;
  label: string;
  host: string;
  port: number | null;
  identityFile: string;
  jumpHost: string;
  selectedDisks: string[];
  selectedInterfaces: string[];
  selectedGpus: string[];
  hiddenMetrics: string[];
  order: number;
}
export interface PriceOverride {
  model: string;
  catalogId: string;
  input: number | null;
  output: number | null;
  cacheRead: number | null;
  cacheWrite: number | null;
}
export interface Settings {
  sources: Source[];
  servers: Server[];
  accounts: Account[];
  theme: string;
  pollSeconds: number;
  quotaSeconds: number;
  concurrency: number;
  paused: boolean;
  priceOverrides: PriceOverride[];
  rawHours: number;
  minuteDays: number;
  hourDays: number;
}
export interface Usage {
  timestamp: number;
  agent: string;
  model: string;
  device: string;
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  cacheKnownInput: number;
  requests: number;
  cost: number;
  pricedTokens: number;
}
export interface Quota {
  id: string;
  provider: string;
  label: string;
  accountId: string | null;
  device: string;
  status: string;
  error: string | null;
  updatedAt: number | null;
  attemptedAt: number;
  windows: {
    id: string;
    label: string;
    usedPercent: number;
    resetsAt: number | null;
    durationMinutes: number | null;
  }[];
  resetCards: {
    availableCount: number;
    credits:
      | {
          id: string;
          title: string | null;
          status: string;
          expiresAt: number | null;
        }[]
      | null;
  } | null;
  balance: {
    is_available: boolean;
    balance_infos: {
      currency: string;
      total_balance: string;
      granted_balance: string;
      topped_up_balance: string;
    }[];
  } | null;
}
export interface DeviceMetric {
  id: string;
  [key: string]: string | number | boolean | null | undefined;
}
export interface Sample {
  timestamp: number;
  hostname: string;
  uptime: number;
  bootId: string;
  cpu: {
    usage: number | null;
    user: number | null;
    system: number | null;
    ioWait: number | null;
  };
  cores: DeviceMetric[];
  load: number[];
  memory: {
    total: number;
    used: number;
    available: number;
    cached: number;
    swapTotal: number;
    swapUsed: number;
  };
  network: DeviceMetric[];
  disks: DeviceMetric[];
  diskIo: DeviceMetric[];
  gpus: DeviceMetric[];
  gpuProcesses: {
    gpu: string;
    pid: string;
    name: string;
    memory: number | null;
  }[];
  gpuError: string | null;
  diskError: string | null;
  sampleCount: number;
}
export interface ServerState {
  id: string;
  device?: string;
  label: string;
  status: string;
  updatedAt?: number;
  attemptedAt?: number;
  error?: string;
  sample?: Sample | null;
  collector?: { version: number; arch: string; lastSample: number };
}
export interface Snapshot {
  settings: Settings;
  device: string;
  usage: Usage[];
  sources: {
    source: string;
    status: string;
    scannedAt: number;
    events: number;
    errors: number;
    message: string;
  }[];
  quotas: Quota[];
  servers: ServerState[];
  priceUpdatedAt: number | null;
  priceError: string | null;
  generatedAt: number;
}
export interface History {
  resolution: number;
  rows: { timestamp: number; payload: Sample }[];
}
