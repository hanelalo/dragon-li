# Tasks: change-04-provider-settings-ui-core

- [x] 新建 `SettingsPage` 与 profile 列表/编辑组件。
- [x] 接入 `config_get`、`config_save`、`config_check_external_change`、`config_apply_external_change`。
- [x] 实现 profile 新增、编辑、删除、启用/禁用。
- [x] 实现表单校验与错误码提示映射。
- [x] 实现“测试连接”动作并复用 `chat_send` 完成连通性校验。
- [x] 实现测试结果反馈（成功、失败错误码、request_id）。
- [x] 保存成功后同步可选 profile 到全局状态。
- [x] 完成手动回归：正常保存、非法 JSON/schema、外部变更确认。

## 验收清单

- [x] UI 内可完成 profile 全生命周期管理。
- [x] 每个启用 profile 可在设置页完成连通性测试并拿到明确结果。
- [x] 配置异常不会污染已生效配置。
- [x] 保存后聊天页可直接使用新 profile。
