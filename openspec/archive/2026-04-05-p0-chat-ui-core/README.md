# P0 Chat UI Core

本目录归档了完成于 2026-04-05 的 `change-03-conversation-ui-core`。
该变更成功建立了聊天主界面与会话管理界面，打通了“会话选择 -> 发消息 -> 流式回复 -> 历史回看 -> 失败重试”的 UI 闭环。

包含以下组件实现：
- `ChatPage.vue`
- `SessionSidebar.vue`
- `MessageTimeline.vue`
- `Composer.vue`