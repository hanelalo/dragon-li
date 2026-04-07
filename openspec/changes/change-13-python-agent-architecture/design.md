# Design: change-13-python-agent-architecture

## 1. 整体通信架构 (Rust ↔ Python via UDS)

为了确保桌面端的安全性与规避本地端口冲突，放弃 `127.0.0.1` 端口绑定，改用 Unix Domain Sockets (UDS)。

1. **Rust 启动 Python 子进程**：
   - Rust 启动时获取系统缓存目录，生成唯一的 socket 路径（如 `~/.dragon-li/run/agent.sock`）。
   - 通过 `Command` 拉起 `python3 agent/runtime_agent.py --uds <socket_path>`。
   - 必须配置 `.kill_on_drop(true)` 防范僵尸进程。
   - 捕获子进程的 `stdout` 和 `stderr` 并重定向至 Rust 的全局日志系统中，解决日志黑洞问题。
2. **健康检查 (Health Probe)**：
   Rust 内部的 HTTP Client 需要支持 UDS 传输层，循环 `GET http://localhost/health` (走 Socket 隧道) 直到返回 `200 OK`，判定 Agent 启动成功。

## 2. Python Agent 层设计 (`runtime_agent.py`)

Python 侧使用 `FastAPI` + `uvicorn` 构建异步非阻塞服务。
- 解析 CLI 参数 `--uds`，通过 `uvicorn.run(app, uds=args.uds)` 在对应的 Socket 文件上启动服务。
- 实现 `GET /health` 端点，返回简单的 `{"status": "ok"}` 供探活。
- 添加进程平滑退出的信号监听机制。

## 3. Rust 宿主层管控 (`runtime.rs`)

- **`runtime.rs` 修改**：
  重构 `AgentManager::start`：不再是空转脚本，而是带有 UDS 路径传递、僵尸防范和日志流接管的完整生命周期管理。
  重构 `AgentManager::health_check`：抛弃慢速的 CLI 执行，改为通过 `reqwest` 或支持 Socket 通信的 `hyper` 进行本地长连接的健康检测。
