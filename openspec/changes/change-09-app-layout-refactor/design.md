# Change 09: App Layout Refactor Design

## 1. 布局结构对比

### 1.1 修改前 (Before)
```text
+----------------+-------------------+-----------------------------------+
|   Global Nav   | Conversation List |          Chat Area                |
| - Chat         | - Session 1       |                                   |
| - Config       | - Session 2       |                                   |
| - Memory       |                   |                                   |
+----------------+-------------------+-----------------------------------+
```

### 1.2 修改后 (After)
```text
+-------------------+----------------------------------------------------+
| Conversation List |                    Chat Area                       |
|                   |                                                    |
| - Session 1       |                                                    |
| - Session 2       |                                                    |
|                   |                                                    |
|                   |                                                    |
| [⚙️ Settings]     |                                                    |
+-------------------+----------------------------------------------------+
```

## 2. 路由结构调整 (Vue Router)

- **移除**：原有的包含 Global Nav 的 Layout 壳组件。
- **调整**：`/` 默认路由直接指向包含 `Conversation List` 和 `Chat Area` 的组件。
- **新增/调整**：
  - 将配置页 (`/config`) 和记忆页 (`/memory`) 改为覆盖在当前聊天页面之上的弹窗/抽屉组件，或者作为覆盖全屏的新路由（视具体 UI 库和体验而定，建议采用全屏路由覆盖并带有返回按钮）。

## 3. 组件重构细节

### 3.1 会话列表底部 (ConversationList.vue)
- 在组件底部（Flex column, justify-content: space-between 的底部或 footer 区域）增加一个常驻操作栏。
- 包含一个 `Settings` (齿轮) 图标按钮。
- 点击该按钮触发路由跳转或状态变更，打开设置/记忆聚合面板。

### 3.2 设置入口聚合面板 (Settings Panel)
当用户点击齿轮按钮后，展示一个聚合入口（可以是下拉菜单，或者直接跳转到一个新的综合设置页面），包含：
- **API 配置 (Config / Provider Settings)**
- **记忆管理 (Memory Center)**

建议：可以直接跳转到一个带有左侧标签页的统一 `SettingsView`，将配置和记忆作为子 Tab。

## 4. 样式调整 (CSS)

- 调整主应用的 CSS Grid/Flex 布局，去掉左侧导航的宽度（例如原先的 `64px` 或 `80px`）。
- 确保会话列表在去掉左侧导航后，视觉重心依然平衡，边界阴影或分割线处理得当。
