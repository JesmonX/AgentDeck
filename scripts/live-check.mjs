// Read-only smoke check against pnpm dev; never edits settings or queries credentials.
import { chromium } from "playwright";
import fs from "node:fs";
const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.CHROME_PATH || undefined,
  args: ["--no-sandbox"],
});
try {
  const page = await browser.newPage({
    viewport: { width: 1440, height: 1100 },
  });
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto("http://127.0.0.1:1420/");
  await page
    .getByRole("heading", { name: "Agent 概览" })
    .waitFor({ timeout: 60000 });
  await page.waitForTimeout(1000);
  const result = await page.evaluate(async () => {
    const response = await fetch("/api", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ method: "snapshot", params: {} }),
    });
    const { result: s, error } = await response.json();
    if (error) throw new Error(error);
    return {
      sources: s.sources.map((x) => ({
        source: x.source,
        status: x.status,
        events: x.events,
        errors: x.errors,
      })),
      usageBuckets: s.usage.length,
      priceCache: !!s.priceUpdatedAt,
      quotas: s.quotas.map((q) => ({
        provider: q.provider,
        status: q.status,
        windows: q.windows.length,
        resetCards: q.resetCards?.availableCount,
      })),
    };
  });
  if (!result.usageBuckets) throw new Error("No real usage loaded");
  if (errors.length) throw new Error(errors.join("\n"));
  fs.mkdirSync("artifacts", { recursive: true });
  await page.screenshot({ path: "artifacts/agents-live.png", fullPage: true });
  console.log(
    "PASS real backend + real usage browser smoke:",
    JSON.stringify(result),
  );
  const denied = await page.request.post("http://127.0.0.1:1420/api", {
    headers: {
      Origin: "https://untrusted.invalid",
      "Content-Type": "application/json",
    },
    data: { method: "settings", params: {} },
  });
  const response = await denied.json();
  if (!response.error) throw new Error("Cross-origin request was not rejected");
  console.log("PASS cross-origin request rejected");
} finally {
  await browser.close();
}
