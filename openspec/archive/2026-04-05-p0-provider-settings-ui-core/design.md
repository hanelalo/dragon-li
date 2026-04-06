# Design: change-04-provider-settings-ui-core

## 页面结构

- `SettingsPage`
  - `ProfileList`：展示 profile 概览与状态。
  - `ProfileEditor`：编辑基础字段（provider/base_url/api_key/model/enabled）。
  - `ValidationPanel`：展示 schema 校验结果与连通性结果。

## 交互流程

1. 进入配置页调用 `config_get` 拉取当前配置。
2. 用户编辑后先前端校验，再调用 `config_save`。
3. 若检测到外部变更，提示用户确认后调用 `config_apply_external_change`。
4. 保存成功后刷新全局 `activeProfileId` 可选项。

## 连通性测试设计

- 在 `SettingsPage` 增加“测试连接”操作，基于当前选中 profile 发起最小化请求。
- 调用方式：复用 `chat_send`，使用固定测试 prompt 与独立 `request_id`。
- 成功判定：返回 `ok=true` 且有 `done` 事件。
- 失败判定：展示 `PROVIDER_*` / `CONFIG_*` 错误码与可读提示。
- 测试请求不写入会话消息列表，仅保留 request log 追踪。

## 数据约束

- `api_key` 仅在输入阶段可见，列表中遮罩显示。
- `base_url` 必须为 `https`。
- `id` 必须唯一且不可为空。

## 错误处理

- 直接映射 `CONFIG_*` 错误码到可操作提示。
- 保存失败保持编辑态，不丢用户输入。

## 风险与缓解

- 风险：外部配置变更与本地编辑冲突。
- 缓解：冲突时阻止直接覆盖，强制用户选择“刷新后再编辑”。
- 风险：连通性测试污染聊天数据。
- 缓解：测试请求使用独立 `session_id` 或空会话并在 UI 层明确隔离展示。
