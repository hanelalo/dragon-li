# Tasks: change-20-python-search-tool

- [x] **依赖配置**：更新 `agent/requirements.txt` 增加 `httpx` 依赖。
- [x] **API 封装**：编写 `execute_web_search` 函数，处理 Brave API 请求与数据清洗。
- [x] **Schema 注入**：在 `_openai_stream` 中动态注入符合 OpenAI 规范的 `web_search` 的 tool schema。
- [x] **Schema 注入**：在 `_anthropic_stream` 中动态注入符合 Anthropic 规范（使用 `input_schema`）的 `web_search` 的 tool schema。
- [x] **执行拦截**：修改两个 stream 函数中的 tool 执行循环，拦截 `web_search` 并调用本地函数。
- [x] **测试验证**：分别使用 OpenAI 和 Anthropic 模型，开启搜索开关，验证是否能跑通闭环并给出正确回答。