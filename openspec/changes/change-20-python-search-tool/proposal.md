# Proposal: change-20-python-search-tool

## 核心目标
在 Python Agent 端实现 `web_search` 工具，调用 Brave API，进行数据清洗，并处理 OpenAI/Anthropic 两种 Provider 的 Tool Call 闭环逻辑。

## 背景与痛点
这是网络搜索的核心能力。直接返回 Brave API 的原始大 JSON 会消耗大量 Token。此外，当前 `llm_provider.py` 主要处理 MCP 工具，我们需要将内部的 `web_search` 统一接入现有的工具调度循环中。

## 解决方案
1. **工具实现**：编写基于 `httpx` 的请求函数，清洗结果仅保留前 5 条的 `title`, `url`, `description`。
2. **上下文注入**：如果 `req.enable_web_search` 为 True 且配置了 Key，在 `_openai_stream` 和 `_anthropic_stream` 中将 `web_search` schema 手动附加到 `tools` 列表中。
3. **执行闭环**：拦截 `tool_calls`，如果名字是 `web_search` 则执行本地函数，否则执行 `mcp_manager.call_tool`。将结果追加到 `messages` 继续流式生成。

## 验收标准
- 开启搜索并提问需要实时信息的问题。
- Python 端正确触发 Brave API 调用，返回清洗后的精简 JSON。
- OpenAI 和 Anthropic 两种模型均能正确识别工具调用、接收结果并生成最终回答。