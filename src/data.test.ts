import { describe, it, expect } from "vitest";
import {
  sumUsage,
  byModel,
  localDay,
  dayStart,
  visibleNetwork,
  historySeries,
} from "./data";
import type { Usage, History } from "./types";
const row = (model: string, input: number, cacheRead: number): Usage => ({
  model,
  input,
  cacheRead,
  output: 10,
  cacheWrite: 0,
  cacheKnownInput: input,
  cost: 0,
  pricedTokens: 0,
  requests: 1,
  agent: "codex",
  device: "a",
  timestamp: 0,
});
describe("display aggregates", () => {
  it("keeps missing rates and collection gaps unknown while respecting selected interfaces", () => {
    const history = {
      resolution: 5,
      rows: [
        {
          timestamp: 100,
          payload: {
            network: [
              { id: "eth0", rxRate: null },
              { id: "lo", rxRate: 50, defaultVisible: false },
            ],
          },
        },
        {
          timestamp: 200,
          payload: {
            network: [
              { id: "eth0", rxRate: 10 },
              { id: "lo", rxRate: 60, defaultVisible: false },
            ],
          },
        },
      ],
    } as unknown as History;
    expect(historySeries(history, "rx")).toEqual([
      [100000, null],
      [105000, null],
      [200000, 10],
    ]);
    expect(historySeries(history, "rx", ["lo"])).toEqual([
      [100000, 50],
      [105000, null],
      [200000, 60],
    ]);
  });
  it("weights cache ratio by tokens, never averaging request percentages", () => {
    const s = sumUsage([row("a", 100, 80), row("a", 900, 90)]);
    expect(s.cacheRead / s.cacheKnownInput).toBe(0.17);
  });
  it("keeps unknown prices separate from zero cost", () => {
    const m = byModel([row("unknown", 100, 0)])[0];
    expect(m.pricedTokens).toBe(0);
    expect(m.input + m.output).toBe(110);
  });
  it("round-trips local calendar boundaries", () => {
    expect(localDay(dayStart("2026-10-02"))).toBe("2026-10-02");
  });
  it("selects interfaces without double-counting virtual defaults", () => {
    const n = [
      { id: "eth0", defaultVisible: true },
      { id: "lo", defaultVisible: false },
    ];
    expect(visibleNetwork(n, []).length).toBe(1);
    expect(visibleNetwork(n, ["lo"])[0].id).toBe("lo");
  });
});
