AgentDeck 首个 preview：Agent 用量与服务器资源监控。

- Codex / Claude Code / Antigravity token、缓存率、模型分布、长期热力图与 OpenRouter 等价费用。
- 账户额度、重置时间、Codex 重置卡只读查看，以及 DeepSeek 余额。
- Linux SSH 采集器、服务器指标细分、历史记录与断线补传。
- Windows x64、macOS Intel / Apple Silicon、Linux x64 安装包；附 Linux x64 / ARM64 独立采集器。

下载与你系统对应的安装文件；`agentdeck-collector-*` 是远端 Linux 命令行采集器。校验值见 `SHA256SUMS`。

这是未签名、未公证的 preview。CI 编译和打包成功不代表已经在各平台完成安装、托盘和凭据库验收。Claude 需要有效登录凭据，DeepSeek 需要自行配置 Key，SSH 与 NVIDIA 需连接实际设备验证。详见仓库 README 与 VALIDATION.md。

为快速迭代，本发布任务执行类型检查、原生编译、打包与产物完整性校验；完整测试可单独运行 Verify workflow。
