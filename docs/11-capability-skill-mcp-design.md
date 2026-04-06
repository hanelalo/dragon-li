# 11. Capability / Skill / MCP 预埋设计

目标：在 MVP 阶段不启用高风险扩展执行能力，但在架构与数据模型上为后续 `Skill` 与 `MCP` 留出稳定接入点，避免重构。

## 设计原则

- 统一抽象：所有可调用能力统一为 `Capability`。
- 默认关闭：`Skill` 与 `MCP` 在 MVP 默认禁用执行。
- 显式授权：执行前必须通过权限校验与用户确认。
- 最小耦合：UI 与 Agent 只依赖统一能力接口，不依赖具体实现。

## 能力抽象模型

`Capability` 分三类：

- `native`：内置能力（会话、记忆、配置等）。
- `skill`：本地技能能力（基于 skill 定义）。
- `mcp`：通过 MCP 连接器暴露的外部能力。

建议字段：

- `id`
- `type` (`native`/`skill`/`mcp`)
- `name`
- `description`
- `input_schema_json`
- `risk_level` (`low`/`medium`/`high`)
- `enabled`
- `created_at`
- `updated_at`

## 统一调用协议（Agent 内部）

- `list_capabilities()`
- `get_capability_schema(capability_id)`
- `dry_run_capability(capability_id, input)`
- `invoke_capability(capability_id, input)`

说明：

- `dry_run` 用于展示预期行为、权限需求、影响范围。
- `invoke` 必须经过权限与开关校验。

## 权限与安全策略

- 默认全部禁用：`skill`、`mcp` 初始 `enabled=false`。
- 授权粒度：按 capability 单独授权（一次性 / 会话级 / 永久）。
- 权限声明维度：
  - 文件访问范围
  - 网络访问范围
  - 数据写入范围
  - 命令执行权限（MVP 禁止）
- 与安全边界联动：任何越过 `~/.dragon-li/` 或白名单域名的请求直接拒绝。

## Skill 预埋策略（MVP）

- 支持能力发现与展示，不开放实际执行。
- UI 可展示：名称、描述、输入参数、风险级别、当前开关状态。
- 可保留“启用”入口，但默认关闭，且提示“后续版本开放”。

## MCP 预埋策略（MVP）

- 支持连接器配置与状态展示，不开放真实调用。
- 建议管理字段：
  - `connector_id`
  - `name`
  - `endpoint`
  - `status` (`configured`/`healthy`/`error`/`disabled`)
  - `allowed_domains_json`
  - `enabled`
  - `updated_at`

## 数据模型建议（SQLite）

可新增以下表（MVP 可先建表不用）：

- `capabilities`
- `capability_permissions`
- `capability_invocations`
- `mcp_connectors`

## 配置文件建议

- `~/.dragon-li/config/capabilities.json`

用途：

- 记录全局开关、默认禁用策略、白名单配置。

## 分阶段路线

- MVP：仅完成抽象层、数据结构、UI 展示入口；执行默认关闭。
- Phase 2：开放受控 `skill` 执行（白名单 + 显式授权）。
- Phase 3：开放受控 `mcp` 调用（连接器健康检查 + 域名白名单 + 审计日志）。
