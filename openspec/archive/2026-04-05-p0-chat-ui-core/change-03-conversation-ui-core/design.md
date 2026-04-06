# Design: change-03-conversation-ui-core

## 页面结构

- `ChatPage`
  - `SessionSidebar`：会话列表与管理操作。
  - `MessageTimeline`：消息渲染与滚动控制。
  - `Composer`：输入发送与重试。

## 状态设计

- 页面级状态：
  - `sessionList`
  - `activeSessionId`
  - `messagesBySession`
  - `sendingState`
  - `lastError`
- 全局状态仅保存跨页共享字段，避免聊天态被误放到全局。

## 交互流

1. 进入聊天页加载会话列表。
2. 在会话侧栏支持重命名，提交后更新本地列表与数据库记录。
3. 选择会话后加载消息历史。
4. 发送消息触发 `chat_send`，订阅流事件更新最后一条 assistant 消息。
5. `done` 事件后落盘并解除输入禁用。
6. 失败时展示错误码与 `request_id`，允许“一键重试”。

## 一致性策略

- 发消息前本地先插入“streaming 占位消息”。
- 请求失败时更新同条消息状态为 `failed`，避免重复插入。
- 会话重命名采用“后端成功后再提交 UI 更新”，避免本地与数据库标题不一致。

## 风险与缓解

- 风险：流事件乱序造成 UI 抖动。
- 缓解：按 `request_id` + 本地消息上下文串行消费事件。
