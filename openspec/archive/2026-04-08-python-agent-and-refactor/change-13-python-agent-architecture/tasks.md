# Tasks: change-13-python-agent-infrastructure

## Python 端服务建设
- [x] 在 `agent/` 目录下创建 `requirements.txt`，添加 `fastapi` 和 `uvicorn`。
- [x] 重构 `agent/runtime_agent.py`，使用 FastAPI 搭建基础应用，实现 `GET /health` 端点返回 `{"status": "ok"}`。
- [x] 在 CLI 入口中，使用 `argparse` 解析 `--uds` 参数，并通过 `uvicorn.run(app, uds=args.uds)` 启动基于 Unix Domain Socket 的服务（Windows 环境可使用 Named Pipes 或回退方案）。

## Rust 宿主层进程管控与安全改造
- [x] 修改 `src-tauri/src/runtime.rs` 中的 `AgentManager::start`：
  - 生成唯一的 UDS 路径（如 `~/.dragon-li/run/agent.sock`）。
  - 修改拉起 Python 进程的参数为 `["runtime_agent.py", "--serve", "--uds", "<socket_path>"]`。
  - 为 `Command` 配置 `.kill_on_drop(true)` 防范僵尸进程。
  - 捕获 `stdout` 和 `stderr`，将其重定向至 Rust 的 `tracing` 日志系统中，消除日志黑洞。
  - 将生成的 UDS 路径保存到 `AgentManager` 的状态中。
- [x] 修改 `AgentManager::health_check`：
  - 移除原有的通过 CLI 执行 `--health-check` 的逻辑。
  - 使用支持 Unix Socket 的 HTTP Client（如配置了 socket 连接器的 `reqwest`），向该 socket 路径发起 `GET /health` 请求。如果返回 200 OK 则判定 Agent 启动成功。

## 验收清单
- [x] 启动 Dragon Li 应用后，Rust 能够在后台成功拉起 Uvicorn 服务，且通过 UDS 进行通信，不暴露任何本地 TCP 端口。
- [x] 强制结束 Rust 宿主进程后，后台的 Python `runtime_agent.py` 进程能够被自动终止，不留僵尸进程。
- [x] Python 进程打印的任何 `print()` 或异常堆栈，都能在 Rust 的终端或应用日志文件中被看到。