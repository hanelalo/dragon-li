# 09. 错误码与重试策略

本文定义桌面应用（Rust 宿主 + Python Agent）的统一错误码、UI 反馈和重试行为。

## 错误返回结构（建议）

```json
{
  "ok": false,
  "error": {
    "code": "PROVIDER_TIMEOUT",
    "message": "Request timed out after 30s",
    "retryable": true
  },
  "meta": {
    "request_id": "req_xxx",
    "ts": "2026-04-05T14:10:00+08:00"
  }
}
```

当前实现补充：

- `chat_send` 失败时，`error.message` 统一追加 `request_id`，格式：`... [request_id=xxx]`。
- 失败请求（含配置早退失败）会写入 `request_logs`，保证可按 `request_id` 追踪。

## 错误码清单（MVP）

### Provider 相关

- `PROVIDER_AUTH_FAILED`：API Key 无效、缺失或权限不足。
- `PROVIDER_RATE_LIMITED`：触发限流（429）。
- `PROVIDER_TIMEOUT`：请求超时。
- `PROVIDER_UNREACHABLE`：网络不可达或 DNS 失败。
- `PROVIDER_BAD_REQUEST`：请求参数不合法（4xx）。
- `PROVIDER_SERVER_ERROR`：上游服务异常（5xx）。

### 配置相关

- `CONFIG_NOT_FOUND`：配置文件不存在。
- `CONFIG_INVALID_JSON`：JSON 格式错误。
- `CONFIG_SCHEMA_INVALID`：字段缺失或类型不匹配。
- `CONFIG_PROFILE_NOT_FOUND`：指定 profile 不存在或已禁用。
- `CONFIG_RELOAD_REJECTED`：检测到外部配置变更，但用户未确认应用。

### 数据存储相关

- `DB_INIT_FAILED`：SQLite 初始化失败。
- `DB_BUSY`：数据库锁冲突或繁忙。
- `DB_WRITE_FAILED`：写入失败。
- `DB_READ_FAILED`：读取失败。
- `MEMORY_FILE_WRITE_FAILED`：Markdown 记忆写入失败。
- `MEMORY_FILE_READ_FAILED`：Markdown 记忆读取失败。

### 业务流程相关

- `SESSION_NOT_FOUND`：会话不存在。
- `MESSAGE_NOT_FOUND`：消息不存在。
- `MEMORY_CANDIDATE_NOT_FOUND`：候选记忆不存在。
- `MEMORY_CONFLICT_DETECTED`：候选与既有记忆冲突。
- `INVALID_REQUEST`：通用参数校验失败。
- `INTERNAL_ERROR`：未分类内部错误。

## 重试策略（MVP）

最终拍板参数：

- 自动重试错误：`PROVIDER_TIMEOUT`、`PROVIDER_UNREACHABLE`、`PROVIDER_SERVER_ERROR`、`DB_BUSY`
- 最大重试次数：2 次
- 退避策略：指数退避（`500ms -> 1500ms`）
- 每次尝试请求超时：30s
- 不自动重试：`PROVIDER_AUTH_FAILED`、`PROVIDER_BAD_REQUEST`、`PROVIDER_RATE_LIMITED`、`CONFIG_INVALID_JSON`、`CONFIG_SCHEMA_INVALID`、`INVALID_REQUEST`

### 自动重试（允许）

- `PROVIDER_TIMEOUT`
- `PROVIDER_UNREACHABLE`
- `PROVIDER_SERVER_ERROR`
- `DB_BUSY`

策略建议：

- 最大重试次数：2 次
- 退避策略：指数退避（例如 500ms -> 1500ms）
- 每次尝试请求超时：30s
- 每次重试写入 `request_logs`

### 不自动重试（禁止）

- `PROVIDER_AUTH_FAILED`
- `PROVIDER_BAD_REQUEST`
- `PROVIDER_RATE_LIMITED`
- `CONFIG_INVALID_JSON`
- `CONFIG_SCHEMA_INVALID`
- `INVALID_REQUEST`

策略建议：

- 直接向用户展示可操作提示（如“请检查 API Key”）。
- 保留“重试”按钮，但不自动重试。

## UI 反馈规范

- 可重试错误：提示条显示“请求失败，可重试”，展示 `request_id`。
- 不可重试错误：提示明确下一步动作（如“前往设置页修复配置”）。
- `PROVIDER_RATE_LIMITED`：提示“请求过于频繁，请稍后再试或切换模型/API 配置”。
- 配置文件解析失败：在设置页显示具体字段错误，阻止保存。
- 长任务失败：在会话消息流中插入失败卡片，支持“一键重试”。
- 配置热更新：检测到外部改动后显示确认提示，用户确认后才应用新配置。

### 流式事件补充（chat_stream_event）

- `delta`：增量文本。
- `done`：正常结束。
- `aborted`：异常终止，字段为 `code`、`message`、`retryable`。

## 日志与排障

- 所有错误必须记录到 `request_logs` 或应用日志。
- 日志至少包含：`request_id`、`error_code`、`provider`、`model`、`session_id`、`created_at`。
- 对用户展示的信息应简洁；内部日志可保留详细异常栈。

当前实现补充：

- `chat_send` 成功日志：`status=ok`，包含 `provider/model/session_id/request_id/latency_ms`。
- `chat_send` 失败日志：`status=failed`，包含 `error_code` 与上述追踪字段。
- memory 注入阶段失败会额外写日志：`status=memory_injection_failed`，并保留 `request_id` 与错误码。
