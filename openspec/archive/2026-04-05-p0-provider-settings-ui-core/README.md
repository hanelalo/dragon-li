# P0 Provider Settings UI Core

本目录归档了完成于 2026-04-05 的 `change-04-provider-settings-ui-core`。
该变更成功建立了 Provider 配置页面闭环，支持 profile 的新增、编辑、删除、启用/禁用，以及配置校验与连通性测试。

包含以下组件与功能实现：
- `SettingsPage` 与 profile 列表/编辑组件
- 接入配置相关的 API (`config_get`, `config_save`, `config_check_external_change`, `config_apply_external_change`)
- 完整的表单校验与错误提示
- 连通性测试动作及结果反馈机制