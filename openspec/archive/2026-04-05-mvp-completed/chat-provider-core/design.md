# Design: chat-provider-core

- Prompt 分层：System/Runtime/Memory/User。
- 统一响应结构与错误码。
- 自动重试：timeout/unreachable/5xx/db_busy，2 次，500ms->1500ms，30s/尝试。
- rate_limited 直接提示用户，不自动重试。
