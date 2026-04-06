# OpenSpec Workspace

本目录用于按 OpenSpec（spec-driven）方式推进 `Dragon-Li`。

## 原则

- 每个 change 只解决一个清晰问题，避免“大而全”。
- 每个 change 必须可在短周期内完成并验收。
- 跨模块改动可拆成多个独立 change，按依赖顺序推进。

## 工作流

1. 在 `changes/<change-id>/proposal.md` 写变更动机与范围。
2. 在 `changes/<change-id>/design.md` 写设计细节与数据流。
3. 在 `changes/<change-id>/tasks.md` 写可执行任务与验收条件。
4. 评审通过后进入实现。
5. 任务闭环后，将相关的核心设计提炼、更新并沉淀到全局的 `specs/` 目录中。
6. 最后将该 `change` 移动至 `archive/` 归档。

## 全局规范 (Specs)

MVP 阶段提炼的核心系统规范已沉淀至 `specs/` 目录，作为后续开发的基线（Baseline）：
- [`architecture.md`](./specs/architecture.md): 系统整体架构（Tauri + Vue + SQLite）
- [`chat_engine.md`](./specs/chat_engine.md): 对话引擎（流式解析、Token统计、并发自动标题）
- [`memory_system.md`](./specs/memory_system.md): 长效记忆系统（启发式提取、审批流、TF-IDF 倒排索引）

## 已完成变更 (Completed)

- [x] **`change-09-app-layout-refactor`** - 重构 App 布局，移除左侧全局菜单，改为双栏聊天主导，新增底部悬浮抽屉配置入口（归档于 `archive/2026-04-06-change-09-app-layout-refactor`）
- [x] **`change-05-p0-e2e-smoke-and-acceptance`** - 完成 P0 主链路端到端验收与测试用例收口（归档于 `archive/2026-04-06-change-05-e2e`）
- [x] **`change-04-provider-settings-ui-core`** - 建立 Provider 配置页面闭环，支持 profile 管理与连通性验证（归档于 `archive/2026-04-05-p0-provider-settings-ui-core`）
- [x] **`change-03-conversation-ui-core`** - 实现侧边栏会话列表、聊天流式渲染与输入框（归档于 `archive/2026-04-05-p0-chat-ui-core`）
- [x] **`change-01-app-shell-and-state`** - 初始化 Vue 应用外壳与全局状态响应（归档于 `archive/2026-04-05-p0-app-shell-and-state`）

## 执行顺序（当前计划）

1. `desktop-runtime-core`
2. `config-and-guardrails-core`
3. `sqlite-conversation-core`
4. `chat-provider-core`
5. `memory-pipeline-core`
6. `memory-center-ui`
7. `macos-packaging`

说明：

- 顺序依据依赖关系与耦合度确定。
- 若某个 change 阻塞严重，可拆出更小子 change，但需保持依赖拓扑不变。
