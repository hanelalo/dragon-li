# Proposal: change-21-search-status-stream

## 核心目标
解决搜索过程中的等待焦虑。在后端执行 Tool Call 时，通过 Stream 事件向前端透传状态，前端在对话流中展示 Loading 提示。

## 背景与痛点
网络搜索带来额外延迟，前端这段时间无任何反馈，用户体验极差。现有的 SSE 流中已有 `delta`、`reasoning` 等事件，我们需要增加工具调用的状态事件，并在 `MessageTimeline.vue` 中渲染。

## 解决方案
1. **事件定义**：在 Python 和 Rust 的 `ChatStreamEvent` 中新增 `tool_call` 类型。
2. **事件下发**：在 Python 端执行 `web_search` 等工具前 yield `status="started"`，执行完毕后 yield `status="finished"`。
3. **前端渲染**：在 Vue 中，当接收到 `started` 事件时，给当前 Message 对象增加临时属性 `tool_status: 'started'`，并在 `MessageTimeline.vue` 中展示“正在搜索网络...”的提示块。

## 验收标准
- 触发搜索时，界面立刻出现“正在搜索网络...”的提示及 Loading 动画。
- 搜索完毕开始输出正文或思考内容时，提示更新为完成状态或隐藏。