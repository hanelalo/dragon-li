# Proposal: change-13-python-agent-infrastructure

## 核心问题
目前 Rust 和 Python 的通信和进程管理极为简陋。Python 只是一个通过 `stdio` 打印 "ok" 的死循环脚本，无法承担后续任何复杂的 AI 交互、数据传输或 MCP 工具执行的职责。
同时，作为一款 Tauri 桌面端应用，我们不能简单地在后台暴露一个 `127.0.0.1` 的 HTTP 端口。这不仅会带来**端口冲突**的极高风险，还存在严重的**本地越权安全漏洞**（任意恶意脚本均可请求该端口触发计费或本地工具调用）。此外，孤儿僵尸进程和日志黑洞也是桌面端应用必须解决的痛点。

## 解决方案
本 Change 作为架构重构的第一步，专注于**安全、高性能的通信通道和严谨的生命周期管理建设**。
1. **Python 端**：引入 FastAPI 框架，将其改造为一个本地的 API 服务器，但**摒弃 TCP 端口绑定，改用 Unix Domain Sockets (UDS) / Named Pipes** 进行通信，从操作系统底层实现安全隔离。
2. **Rust 端**：修改进程管理器，拉起 Python 服务时传入 Socket 路径。接管 Python 进程的标准输出与错误流，统一汇入应用的日志系统，并配置严格的防僵尸进程机制（Kill on Drop）。使用基于 Socket 的 HTTP 探针（`/health`）来判断进程是否就绪。

这一步**不涉及**大模型请求的迁移，而是为后续的高可靠迁移铺平道路。

## 影响范围
- `agent/runtime_agent.py` (引入 FastAPI 和 Uvicorn，支持 `--uds` 参数)
- `agent/requirements.txt`
- `apps/desktop/src-tauri/src/runtime.rs` (重构 AgentManager 的 start, health_check，接管日志与 Socket 通信)