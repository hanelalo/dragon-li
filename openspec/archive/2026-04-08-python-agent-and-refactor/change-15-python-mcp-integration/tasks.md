# Tasks: change-15-python-mcp-integration

## Python 层的 MCP 集成与 Tool Calling (核心目标)
- [x] 在 `agent/requirements.txt` 中添加官方 `mcp` 库。
- [x] 在 `agent/mcp_client.py` 中实现一个基础的 MCP Client Manager：
  - 能够根据传入的参数（`StdioServerParameters`）拉起外部的 Node/Python MCP 服务器进程。
  - 完成握手协议并能够通过 `tools/list` 获取可用的工具集合（Schema）。
- [x] 在 FastAPI 启动生命周期（`@asynccontextmanager`）中，初始化一个本地的 MCP Server 实例（例如用于测试的 `sqlite` 监控服务或 `filesystem`）。
- [x] 修改 `POST /v1/chat/stream` 的 LLM 调用逻辑：
  - 请求大模型前，从 MCP Manager 获取所有的工具列表，并注入到大模型的 `tools` 参数中。
  - 监听大模型的返回流，如果遇到 `tool_calls`（比如调用查天气），则暂停向前端推送字符流。
  - 在 Python 内部调用对应的 MCP Tool 拿到执行结果，作为 `tool`（或者 `function`）消息追加到大模型的上下文中。
  - 携带工具结果再次向大模型发起第二次流式请求，并将大模型的最终总结文本通过 SSE 推回给 Rust。

## 验收清单 (Checklist)
- [x] Python 端能够启动时正确加载配置的 MCP Tools。
- [x] 大模型能够在聊天中识别并触发特定的 Tool Call。
- [x] 前端可以顺滑地看到大模型先调用工具，随后输出基于工具结果生成的回答，整个过程自动在后台串联完成。