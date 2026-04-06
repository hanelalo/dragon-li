# 狸花猫（Dragon-Li）规划文档

本文档用于汇总当前头脑风暴结论，采用「主文件 + 子文件」结构。

## 文档目录

- [01. 产品目标与范围](./01-product-scope.md)
- [02. 桌面架构与技术选型](./02-desktop-architecture.md)
- [03. 会话与记忆设计](./03-session-memory-design.md)
- [04. 配置与 API Profiles](./04-api-profiles-config.md)
- [05. 数据存储与 Schema（SQLite + Markdown）](./05-data-storage-schema.md)
- [06. MVP 迭代计划](./06-mvp-plan.md)
- [07. API Profiles JSON 示例](./07-api-profiles-example.md)
- [08. SQLite 初始化 DDL](./08-sqlite-ddl.md)
- [09. 错误码与重试策略](./09-error-codes-and-retry.md)
- [10. 安全边界细则](./10-security-boundaries.md)
- [11. Capability / Skill / MCP 预埋设计](./11-capability-skill-mcp-design.md)

## 当前状态（摘要）

- 产品定位：可视化桌面 LLM 对话应用（非 CLI）。
- 技术路线：Tauri（Rust）+ Vue 3 + Vite + Python Agent 引擎。
- Provider（MVP）：OpenAI、Anthropic。
- 对话数据：SQLite（`~/.dragon-li/data/dragon_li.db`）。
- 记忆数据：Markdown（`~/.dragon-li/memory/`）。
- 配置策略：`api_profiles.json` 明文文件配置，可在 UI 或文件中修改。
