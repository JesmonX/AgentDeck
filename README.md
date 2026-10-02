# AgentDeck

独立开发的跨平台个人监控应用：Codex / Claude Code / Antigravity 用量、额度、OpenRouter API 等价估价，DeepSeek 余额，以及多台 Linux SSH 服务器的资源与历史。

## 启动

需要 Node.js 24、pnpm 11、Rust stable、Python 3 和系统 OpenSSH。

```sh
pnpm install
pnpm dev
```

访问 **http://127.0.0.1:1420**。此模式启动真实 Rust 后端与仅监听本机的前端；后端通信使用每次启动随机生成的开发令牌。主程序无需开发 HTTP 服务，直接通过 Tauri IPC 通信。

仅查看界面：`pnpm web` 后访问 `http://127.0.0.1:1420/?demo=1`。演示模式明显标注，不访问真实账户、不保存密钥、不执行 SSH 操作。应用不会在真实查询失败时自动切换为演示数据。

桌面运行与安装包：

```sh
pnpm desktop
pnpm desktop:build
```

Linux 桌面构建需要 GTK 3、WebKitGTK 4.1、Ayatana AppIndicator、librsvg 和 patchelf 开发包。Ubuntu 22.04/24.04 可安装：

```sh
sudo apt-get install libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

macOS 需要 Xcode Command Line Tools；Windows 需要 Visual Studio C++ Build Tools、WebView2 和 OpenSSH 客户端。主窗口关闭后进入托盘，托盘“退出”才结束程序。

## 连接数据

- **用量**：设置 → 数据源，默认扫描当前用户的 `.codex/sessions`、`.claude/projects`、`.gemini`。支持自定义绝对路径。首次历史较大时分批索引，后续增量读取；不存储对话正文。
- **Codex**：先在原 CLI 登录，应用通过 `codex app-server` 的只读账户接口获取限额和重置卡。
- **Claude Code**：读取已有 `.claude/.credentials.json` 或 macOS 的 `Claude Code-credentials` 凭据项。OAuth usage 接口需有效登录及 usage/profile 权限；HTTP 401/403 时需要重新登录。应用不擅自轮换原 CLI 的凭据。
- **Antigravity**：先运行 `agy` 登录。通过 `agy -p /usage --output-format json` 查询，要求 CLI 支持非交互 `/usage`。不发送模型推理提示。
- **DeepSeek**：添加账户，然后“设置 Key”，保存到系统凭据库，仅调用官方余额接口。Linux 桌面必须有可用的 Secret Service / 系统凭据服务；凭据库不可用时明确失败，不退回明文存储。
- **价格**：从 OpenRouter 公共模型目录下载，默认每天更新。精确模型 ID 或唯一精确后缀匹配；其他名称通过“模型定价”手动映射。不对未知模型进行猜价。

用量统计与账户额度分开。Token 日志通常不能证明历史账户归属，因此按 Agent、设备、模型展示；额度有明确身份时合并同一账户的快照。未识别的账户不合并。费用为**当前价格下的 API 等价估算**，不是订阅账单。缓存计价字段缺失时相关请求保留未估价状态。

## SSH 服务器

1. 设置 → SSH 服务器，添加配置别名或 `user@host`，保存连接。
2. 核对主机指纹后信任，或使用已有 `~/.ssh/known_hosts`。主机密钥变化不会自动接受。跳板机首次信任请先用系统 SSH 建立连接并核对。
3. 使用私钥、ssh-agent，或将 SSH 密码／私钥口令保存到系统凭据库。
4. 点击“安装采集器”。采集器放在 `~/.local/lib/agentdeck/`，systemd 用户服务为 `agentdeck.service`。不开放公网端口。
5. 如果安装结果报告用户常驻未开启，需要管理员执行 `loginctl enable-linger <用户>`。未满足此条件不能认为登出／重启后的持续记录已就绪。
6. 同步后在服务器页面查看状态和历史。“自定义指标”可选择显卡、网卡、挂载点与各项细分。

跨架构安装需要相应的 Linux 采集器。CI 将 x64 / ARM64 采集器放入每个平台安装包；本地开发可指定构建产物路径。Linux GNU 采集器最低 glibc 版本取决于构建主机，CI 使用 Ubuntu 22.04。

远端每 5 秒采样，默认保留原始数据 48 小时、分钟数据 30 天、小时数据 365 天；桌面连接后同步修改后的保留策略。断线重连按游标拉取，超出变更日志保留期则重新导入仍保留的历史。安装前或采集缺口期间的数据无法补造。

远端 Agent 数据使用 SSH 登录用户的会话目录和登录环境。无需下载其凭据。要查询远端额度，在“账户与额度”选择对应服务器；CLI 不在服务 PATH 时填写绝对可执行路径。

## 独立采集器

```sh
cargo build --release -p agentdeck-cli
target/release/agentdeck-collector version
target/release/agentdeck-collector sample
target/release/agentdeck-collector scan
target/release/agentdeck-collector daemon
```

`sample` / `daemon` 资源采集仅支持 Linux。`AGENTDECK_DATA_DIR` 可覆盖数据库目录；默认遵循操作系统数据目录。数据库内容包含用量元数据和服务器历史，无 API Key。SSH RPC 为带 `version: 1` 的 JSON 请求／响应，仅暴露 capabilities、latest、changes、export、configure、quota。

## 验证与构建

```sh
cargo test -p agentdeck-core -p agentdeck-cli
cargo clippy -p agentdeck-core -p agentdeck-cli --all-targets -- -D warnings
pnpm test
pnpm build
```

浏览器测试：先运行 `pnpm web`，再执行 `pnpm test:browser`；使用 Playwright 已安装的 Chromium，或通过 `CHROME_PATH` 指定本机浏览器。截图保存至 `artifacts/`。

公开仓库：[JesmonX/AgentDeck](https://github.com/JesmonX/AgentDeck)。

`.github/workflows/verify.yml` 在 PR 或手动触发时执行完整检查。推送 `v*-preview.*` 标签会触发 `desktop.yml`：只执行类型检查、编译、打包及产物完整性校验，并发布 GitHub prerelease。支持 Windows x64、macOS Intel / Apple Silicon、Linux x64 安装包与 Linux x64 / ARM64 采集器。Preview 禁用 LTO 以缩短构建时间，不配置签名或公证证书。

```sh
git tag v0.1.0-preview.1
git push origin main v0.1.0-preview.1
```

已有标签构建失败后，可修复代码并推送新的 preview 标签；发布任务也支持输入现有标签手动重跑。

当前执行结果及尚未验证的边界见 [VALIDATION.md](VALIDATION.md)。

当前交付还包括 `artifacts/agentdeck-source-0.1.0.zip` 源码包和 `artifacts/agentdeck-collector-linux-x64` Linux 命令行采集器，校验值见 `artifacts/SHA256SUMS`。三平台桌面安装包需要在相应构建环境生成，不能把前端浏览器测试视为原生桌面验收。
