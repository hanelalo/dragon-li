# Proposal: change-15-python-mcp-integration

## 核心问题
现在我们将 LLM 的请求交给了 Python。如果我们要让 Dragon Li 具备访问外部工具（如查询数据库、读取本地文件）的能力，必须引入 MCP（Model Context Protocol）并允许大模型调用工具（Tool Calling）。

## 解决方案
本 Change 的核心目标是**在 Python 端真正接入官方 `mcp` 库，接管工具调用和复杂编排**。
1. **Python MCP 客户端 (`agent/mcp_client.py`)**：使用 Anthropic 官方的 `mcp` Python 包，实现连接并发现外部服务器的工具列表。
2. **工具挂起与执行 (`agent/llm_provider.py`)**：大模型生成时如果触发 `tool_calls`，Python 会自动挂起流，拦截指令，执行本地 MCP Client 注册的工具，拿到结果后再追加到上下文进行二次推理，最后把最终文本推给 Rust 前端。

## 影响范围
- `agent/mcp_client.py` (新增)
- `agent/llm_provider.py` (增加 tool_calls 处理逻辑)
- `agent/runtime_agent.py` (增加 MCP 服务启动生命周期)