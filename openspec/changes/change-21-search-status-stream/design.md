# Design: change-21-search-status-stream

## 1. 模型扩展
- **Python** (`models.py`):
  ```python
  class ChatStreamEventToolCall(BaseModel):
      type: Literal["tool_call"] = "tool_call"
      tool_call_id: str
      name: str
      status: Literal["started", "finished"]
  ```
  加入 `ChatStreamEvent` Union 中。
- **Rust** (`src-tauri/src/commands/chat.rs`):
  同步增加 `ToolCall { tool_call_id: String, name: String, status: String }` 变体。

## 2. 后端事件下发
在 `llm_provider.py` 中，准备执行工具前：
`yield ChatStreamEventToolCall(tool_call_id=call_id, name=name, status="started")`
执行完毕后：
`yield ChatStreamEventToolCall(tool_call_id=call_id, name=name, status="finished")`

## 3. 前端状态处理
- **`ChatPage.vue`**:
  在 `chat_stream_event` 监听器中，如果收到 `tool_call` 事件，将其维护为一个列表或集合。
  如果是 `started`，将其加入 `targetMsg.active_tools` 数组；如果是 `finished`，将其从该数组中移除。
- **`MessageTimeline.vue`**:
  在 `.content` 区域顶部（`reasoning-block` 上方），增加条件渲染：
  ```html
  <div v-if="msg.active_tools && msg.active_tools.length > 0" class="tool-loading-block">
    <span class="spinner"></span> 正在执行工具调用...
  </div>
  ```
  不需要显示具体的 API 名称，保持对用户的透明度。