# change-02-runtime-chat-orchestration-core

## 背景与问题

当前已有 `chat_send`、Provider adapter、错误码与日志基础，但仍偏“能力可调用”状态，缺少面向产品聊天链路的统一编排约束（profile 选择、prompt 分层、流事件协议一致性、记忆注入入口）。

## 目标

- 固化聊天运行时编排核心协议。
- 明确并稳定 `chat_send` 输入输出契约。
- 确保失败路径可追踪（`request_id` + 错误码 + request_log）。

## 范围

- 统一聊天请求结构（session/profile/model/prompt 层）。
- 固化流式事件与结束语义。
- 串联 request log 与错误码映射。
- 预留并接入记忆注入接口（MVP 关键词检索）。

## 非目标

- 不实现聊天 UI 交互。
- 不实现配置页面可视化操作。

## 依赖与顺序

- 前置依赖：`change-01-app-shell-and-state`。
- 后续 `change-03`（会话与聊天 UI）强依赖本 change 的稳定接口。

## 验收标准

- `chat_send` 在成功/失败场景均返回规范结构。
- 流式事件序列可被前端稳定消费（含终止事件）。
- request_logs 对每次请求有可追踪记录。
