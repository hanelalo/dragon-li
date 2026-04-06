# Design: change-02-runtime-chat-orchestration-core

## 总体设计

以 `chat_send` 为核心编排入口，形成以下处理链：

1. 加载与校验配置（profile 可用性、外部变更检查）。
2. 构建 prompt 分层输入（system/runtime/memory/user）。
3. 调用 Provider adapter（OpenAI/Anthropic）。
4. 输出统一流式事件并写入请求日志。
5. 在失败场景输出统一错误码与可重试标记。

## 接口契约

- 输入：`profile_id`、`request_id`、`session_id`、`model`、`prompt`。
- 输出：`provider`、`model`、`events[]`、`output_text`、`attempts`。
- 错误：复用既有错误码体系（`PROVIDER_*`、`CONFIG_*`、`INVALID_REQUEST`）。

### 冻结字段（实现约束）

- `ChatRequestInput`、`ChatPromptLayer`、`ChatResult` 使用 `deny_unknown_fields`，拒绝未声明字段。
- `request_id`、`profile_id` 必填且非空；`session_id` / `model` 若传入则必须非空字符串。
- `prompt.user` 必填且非空，长度上限 16000 字符。

### `chat_send` 返回结构（当前实现）

- 成功：`{ chat: ChatResult, memory_injection: MemoryInjectionReport }`
- 失败：`ApiResponse.error.message` 统一追加 `request_id`，格式 `... [request_id=xxx]`

## 记忆注入

- 在编排链路中加入 memory 注入步骤（Top-N，带来源 ID）。
- 保持注入可开关与可替换（后续可从关键词检索升级）。

### 注入可观测性

- 注入上限：`MEMORY_INJECTION_TOP_N = 3`。
- 返回 `memory_injection` 报告：
  - `limit`
  - `items[]`（`memory_id`、`summary`、`score`）
  - `error_code`（可空）
  - `error_message`（可空）
- 注入阶段失败不会中断主聊天链路，但会写 `request_logs`（`status=memory_injection_failed`）。

## 审计与追踪

- 每次请求都写 `request_logs`，至少含 `request_id`、状态、错误码、时间戳。
- 流事件通过统一事件名向前端发射。

### 日志一致性语义（当前实现）

- 成功路径写入：`status=ok`，包含 `provider/model/session_id/request_id/latency_ms`。
- Provider/编排失败写入：`status=failed`，包含 `error_code` 与上述追踪字段。
- 配置早退失败（如 `CONFIG_LOCK_FAILED`、`CONFIG_RELOAD_REJECTED`、`CONFIG_NOT_FOUND`、配置加载/检查失败）同样写入 `status=failed`，避免“无日志失败”。
- 任意失败分支都应可通过 `request_id` 查询到至少一条失败日志。

## 流式事件协议

- 事件类型：
  - `delta`：增量文本片段。
  - `done`：正常结束。
  - `aborted`：异常终止，字段为 `code`、`message`、`retryable`。
- Tauri 事件名保持为 `chat_stream_event`，事件载荷包含 `request_id` 与 `event`。

## 风险与缓解

- 风险：配置热更新与聊天请求冲突。
- 缓解：沿用“检测外部变更即拒绝并提示确认”的机制。
