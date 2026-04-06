# Design: change-01-app-shell-and-state

## 总体设计

前端从单页结构升级为：

- `AppShell`：导航、主布局、全局提示位。
- `Router`：页面级路由管理。
- `Store`：全局状态（可先使用 Vue 响应式 store）。
- `Pages`：`ChatPage`、`SettingsPage`、`MemoryPage`。

## 状态模型（最小集）

- `activeRoute`: 当前页面标识。
- `activeSessionId`: 当前会话 ID（供后续聊天页与记忆页复用）。
- `activeProfileId`: 当前 profile ID（供后续配置页与聊天页复用）。
- `runtimeStatus`: 运行时状态（`idle/running/error`）。

## 数据流

1. 应用启动时初始化全局状态。
2. 导航切换只变更路由，不销毁全局状态对象。
3. 子页面通过统一 store 读写共享状态。

## 兼容策略

- 现有 Memory Center 能力迁移到 `MemoryPage`，暂不改变其业务接口。
- 后续 change 通过新增模块扩展，不破坏壳层接口。

## 风险与缓解

- 风险：重构 `App.vue` 时破坏现有功能。
- 缓解：先保留原 Memory 逻辑，做最小拆分，完成后手动回归页面交互。
