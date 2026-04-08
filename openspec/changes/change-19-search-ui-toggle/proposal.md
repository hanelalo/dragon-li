# Proposal: change-19-search-ui-toggle

## 核心目标
在聊天输入框（Composer.vue）增加“网络搜索”的图标开关。实现前置拦截逻辑，未配置 Key 时提示用户，并将开关状态传递给后端。

## 背景与痛点
用户需要自主决定单次对话是否开启联网搜索。为了保持界面整洁，不使用文字标签，仅使用一个网络图标。必须处理好“用户想开启但没配 Key”的边缘场景。

## 解决方案
1. **UI 新增**：在 `Composer.vue` 输入框工具栏增加一个地球/网络 SVG 图标按钮，支持高亮/置灰的 Toggle 状态。
2. **前置拦截**：点击图标时，检查 `appState.settings.braveSearchApiKey`。如果为空，弹出提示并阻止开关激活。
3. **请求传参**：在 `Composer.vue` emit `send` 事件时，带上 `webSearch: isWebSearchEnabled.value`。
4. **接口更新**：在 Rust 和 Python 的 `ChatRequestInput` 中新增 `enable_web_search` 字段。

## 验收标准
- 聊天框有网络图标，点击可切换状态（高亮/置灰）。
- 未配置 Key 时点击图标，弹出提示（例如浏览器 `alert` 或自定义 Toast），且图标不亮。
- 发送消息时，Rust 和 Python 端能正确接收到 `enable_web_search` 的值。