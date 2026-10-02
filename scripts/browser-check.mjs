import { chromium } from "playwright";
import fs from "node:fs";
fs.mkdirSync("artifacts", { recursive: true });
const browser = await chromium.launch({
  headless: true,
  executablePath:
    process.env.CHROME_PATH && process.env.CHROME_PATH !== "bundled"
      ? process.env.CHROME_PATH
      : undefined,
  args: ["--no-sandbox"],
});
const page = await browser.newPage({
  viewport: { width: 1440, height: 1100 },
  deviceScaleFactor: 1,
});
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
for (let retry = 0; ; retry++) {
  try {
    await page.goto("http://127.0.0.1:1420/?demo=1");
    break;
  } catch (e) {
    if (retry === 20) throw e;
    await new Promise((r) => setTimeout(r, 500));
  }
}
await page.getByRole("heading", { name: "Agent 概览" }).waitFor();
await page.waitForTimeout(1000);
await page.screenshot({ path: "artifacts/agents-dark.png", fullPage: true });
await page.getByLabel("Agent 筛选").selectOption("claude");
if ((await page.locator("tbody tr").count()) !== 2)
  throw new Error("Agent filter did not update model table");
await page.getByLabel("Agent 筛选").selectOption("all");
await page.getByLabel("统计时间范围").selectOption("custom");
await page.getByLabel("开始日期").fill("2026-09-01");
await page.getByLabel("统计时间范围").selectOption("7");
await page.getByRole("button", { name: "切换主题" }).click();
await page.waitForTimeout(700);
await page.screenshot({ path: "artifacts/agents-light.png", fullPage: true });
await page.locator("nav").getByRole("button", { name: "服务器" }).click();
await page.getByRole("heading", { name: "服务器", exact: true }).waitFor();
await page.screenshot({ path: "artifacts/servers.png", fullPage: true });
await page.locator(".server-card").first().click();
await page.getByRole("heading", { name: "GPU 工作站", exact: true }).waitFor();
await page.waitForTimeout(400);
await page.screenshot({ path: "artifacts/server-detail.png", fullPage: true });
await page.getByRole("button", { name: "自定义指标" }).click();
await page.getByLabel("GPU 温度", { exact: true }).uncheck();
await page.getByRole("button", { name: "保存显示设置" }).click();
await page.getByRole("dialog").waitFor({ state: "hidden" });
if (await page.getByText("温度 63 °C", { exact: true }).count())
  throw new Error("hidden metric still visible");
await page.locator("nav").getByRole("button", { name: "设置" }).click();
await page.getByRole("button", { name: "账户与额度", exact: true }).click();
await page.getByRole("button", { name: "添加账户", exact: true }).click();
await page.getByLabel("名称", { exact: true }).fill("测试账户");
await page.getByRole("button", { name: "保存账户", exact: true }).click();
await page.getByText("测试账户", { exact: true }).waitFor();
await page.screenshot({ path: "artifacts/settings.png", fullPage: true });
await page.setViewportSize({ width: 1000, height: 850 });
await page.locator("nav").getByRole("button", { name: "Agent 概览" }).click();
await page.screenshot({ path: "artifacts/agents-1000.png", fullPage: true });
const overflow = await page.evaluate(
  () => document.documentElement.scrollWidth > innerWidth,
);
if (overflow) throw new Error("page overflows viewport");
if (errors.length) throw new Error(errors.join("\n"));
console.log(
  "PASS: filters, theme, server details, metric preferences, account editing, 1000px layout; 6 screenshots",
);
await browser.close();
