# Design: config-and-guardrails-core

- 配置文件：`~/.dragon-li/config/api_profiles.json`。
- UI 保存走原子写入。
- 通过显式检查外部变更触发“确认刷新”交互（`check/apply`）。
- 宿主统一做路径与能力越界拦截。
