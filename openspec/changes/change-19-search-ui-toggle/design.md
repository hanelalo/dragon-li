# Design: change-19-search-ui-toggle

## 1. 接口扩展
- **Rust** (`src-tauri/src/commands/chat.rs` 或 `models`): 
  在 `ChatRequestInput` 结构体中新增 `#[serde(default)] pub enable_web_search: bool`。
- **Python** (`agent/models.py`): 
  在 `ChatRequestInput` 中新增 `enable_web_search: bool = False`。

## 2. 前端状态与 UI
- **`Composer.vue`**:
  - 引入全局配置状态：`import { appState } from '../state/appState'`。
  - 定义局部状态：`const isWebSearchEnabled = ref(false)`。
  - 添加图标点击逻辑：
    ```javascript
    function toggleWebSearch() {
      if (!appState.settings.tools?.braveSearchApiKey) {
        alert('请先前往设置页面配置 Brave API Key')
        return
      }
      isWebSearchEnabled.value = !isWebSearchEnabled.value
    }
    ```
  - 在 `<div class="input-container">` 中加入 SVG 按钮，根据 `isWebSearchEnabled` 切换样式（颜色高亮）。
  - 修改 `send` 函数：`emit('send', { text: input.value.trim(), webSearch: isWebSearchEnabled.value })`。（需同步修改父组件的监听）。
- **`ChatPage.vue`**:
  - 接收 `send` 事件时，将 `webSearch` 提取出来，并在构造 `request` 对象时传入 `enable_web_search: webSearch`。