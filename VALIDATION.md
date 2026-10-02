# AgentDeck 0.1.0 验证记录

执行日期：2026-10-02 至 2026-10-03，Linux x64 开发环境。

## 已完成

| 检查 | 结果 | 范围 |
| --- | --- | --- |
| Rust 单元与集成测试 | 15 项通过 | 日志增量恢复、去重、缓存口径、缺失价格、模型匹配、额度解析、账户变更失效、资源计数器、保留策略、同步游标 |
| Rust Clippy / rustfmt | 通过 | core、CLI 所有测试目标；所有 Rust 文件格式 |
| 前端 Vitest | 5 项通过 | 加权缓存率、未知价格、本地日期、网卡选择、采集缺口与未知速率 |
| TypeScript / Vite | 通过 | 类型检查与生产前端构建 |
| Chromium 界面验收 | 通过 | 深浅主题、筛选、日期选择、服务器详情、指标设置、账户表单、1000px 布局 |
| 同步协议集成检查 | 通过 | 505 条记录分页首次同步、重复同步、增量追加、游标失效后的重新导入、资源采样传输 |
| 真实本地页面 | 通过 | 完整 Rust 后端、真实历史用量、价格缓存、额度状态、跨来源请求拒绝 |
| Linux x64 采集器 release 构建 | 通过 | `target/release/agentdeck-collector version` 返回 0.1.0 / protocol=1 |

同步集成检查使用隔离目录和 SSH 传输替身，验证真实采集器的 RPC、存储与桌面同步代码；它不证明真实服务器的身份验证、systemd 安装或重启常驻行为。

## 真实数据与接口

- 本机已导入约 4.4 万条 Codex、1400 多条 Claude Code、6600 多条 Antigravity 用量事件。正在使用的日志会继续增长。来源状态中的 `events` 是本轮扫描数量，不是累计数量。
- Antigravity 有 20 条记录无法确认时间，跳过并显示 partial；不会把未解析记录填成零用量或虚构日期。
- Codex 成功返回 5 小时 / 7 天额度及 2 张重置卡。卡片为只读，可展开查看状态与到期时间。
- Antigravity 成功返回 Gemini 与 Claude/GPT 模型组的 5 小时 / 7 天共 4 个实测窗口。
- OpenRouter 公共价格目录已成功缓存。没有价格的模型保留“未估价”，概览显示已定价 token 覆盖率。
- Claude 额度接口返回 HTTP 401：现有凭据失效，应用保留错误状态，需要用户在原 CLI 重新登录。
- 本机 Linux 资源采样已执行，读到 CPU、内存、磁盘、网卡；当前执行环境未能访问 NVIDIA GPU。
- 约 5.2 万条事件、1.6 万个聚合桶下，价格索引优化后本机单次测量：开发 HTTP 概览约 3.30 秒，release CLI 概览约 0.87 秒；响应约 4.13 MB。这是当前机器测量，不是跨设备性能保证。

## 尚未完成的外部与原生验收

- **macOS / Windows / Linux 桌面安装包没有完成原生验收。** 当前 Linux 缺少 GTK 3、WebKitGTK 4.1 开发包，GLib 版本不足；桌面 `cargo check` 在系统依赖阶段中止。因此 Tauri 主窗口、托盘、原生 IPC、各平台凭据库和安装器仍需在对应系统检查。
- 已提供三平台打包 workflow，但没有上传仓库或执行远程 CI。macOS 签名／公证、Windows 签名未配置。
- 尚未向用户的真实 SSH 服务器安装采集器，也未验证真实跳板机、密码／私钥口令、主机密钥变化、断网恢复、systemd 用户常驻及机器重启场景。
- DeepSeek 尚未配置 API Key，未进行真实余额请求。NVIDIA、AMD、Intel GPU 中，本版只实现 NVIDIA 查询；NVIDIA 仍需在实际显卡服务器上验收。
- Claude OAuth 与 Antigravity 会话数据库属于可能变化的兼容接口；不兼容时显示不可用或部分解析，不推断额度。

## 复现

```sh
cargo fmt --all -- --check
cargo test -p agentdeck-core -p agentdeck-cli --locked
cargo clippy -p agentdeck-core -p agentdeck-cli --all-targets -- -D warnings
python3 scripts/protocol-check.py
pnpm test
pnpm build
```

运行 `pnpm dev` 后，在另一终端执行：

```sh
pnpm test:browser
node scripts/live-check.mjs
```

可通过 `CHROME_PATH` 指定 Chromium。`live-check.mjs` 需要已有真实用量，仅读取数据，不修改配置或凭据；它不适用于没有日志的空白 CI 环境。

截图位于 `artifacts/`：`agents-dark.png`、`agents-light.png`、`servers.png`、`server-detail.png`、`settings.png`、`agents-1000.png` 为演示数据界面；`agents-live.png` 为本机真实数据界面。演示模式不会自动替代失败的真实查询。

本地交付文件：`artifacts/agentdeck-source-0.1.0.zip`、`artifacts/agentdeck-collector-linux-x64`、`artifacts/SHA256SUMS`。源码包不含账户数据、数据库、依赖缓存或真实数据截图。Linux 采集器是命令行程序，不是桌面安装包。

## 数据口径

- 输入 token 包含缓存读取和缓存写入；总 token 为输入 + 输出，缓存不会重复加总。
- 缓存率按已知缓存字段的输入 token 加权，字段缺失不当作零命中。
- 用量以 UTC 分钟保存聚合，前端依本地日期统计，兼容半小时／四十五分钟时区及夏令时。
- 费用采用当前价格目录重估历史用量，属于 API 等价估算，不是订阅账单；对话日志通常无法证明账户归属，历史用量按 Agent／设备／模型呈现。
- 网络日／月累计使用采集器 UTC 日界线，并标注不完整统计；首个采样和计数器重置时的速率未知。历史图保留明显的采集断档。
- 日志镜像按全局事件 ID 去重，保留首次导入的设备归属。删除数据源不删除已累计历史。
