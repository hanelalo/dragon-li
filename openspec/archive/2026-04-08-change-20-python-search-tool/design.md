# Design: change-20-python-search-tool

## 1. 搜索 API 客户端 (`agent/llm_provider.py` 或新增 `tools.py`)
```python
import httpx
import json

async def execute_web_search(query: str, api_key: str) -> str:
    url = "https://api.search.brave.com/res/v1/web/search"
    headers = {"X-Subscription-Token": api_key, "Accept": "application/json"}
    async with httpx.AsyncClient() as client:
        res = await client.get(url, headers=headers, params={"q": query, "count": 5})
        res.raise_for_status()
        data = res.json()
        
    results = data.get("web", {}).get("results", [])[:5]
    cleaned = [{"t": r.get("title"), "u": r.get("url"), "d": r.get("description")} for r in results]
    return json.dumps(cleaned, ensure_ascii=False)
```

## 2. 工具 Schema 注入
在 `_openai_stream` 和 `_anthropic_stream` 中，获取 `mcp_manager.get_all_tools()` 后，如果 `req.enable_web_search` 且有 Key：
- **OpenAI 格式**:
  ```python
  tools.append({
      "type": "function",
      "function": {
          "name": "web_search",
          "description": "Search the web for real-time information.",
          "parameters": {
              "type": "object",
              "properties": {"query": {"type": "string"}},
              "required": ["query"]
          }
      }
  })
  ```
- **Anthropic 格式**: 同理转换为 Anthropic 支持的 `input_schema` 格式。

## 3. 执行拦截
在执行工具调用的循环中：
```python
if name == "web_search":
    result_str = await execute_web_search(args.get("query", ""), req.cfg.tools.brave_search_api_key)
else:
    result = await mcp_manager.call_tool(name, args)
    # ... 原有解析逻辑
```

## 4. 工具 Schema 注入注意事项
在注入 Anthropic 格式的 tool schema 时，务必注意与 OpenAI 的差异。Anthropic 使用平铺的 `input_schema` 而不是包裹在 `function.parameters` 中。必须按照各自的官方规范进行注入。