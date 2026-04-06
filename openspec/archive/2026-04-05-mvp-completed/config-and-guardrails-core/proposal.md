# Proposal: config-and-guardrails-core

## Why

配置与安全边界是所有调用链路的前置约束，必须先落地。

## What

- 实现 `api_profiles.json` 读写与校验。
- 实现配置外部变更检测与用户确认后生效。
- 实现边界校验（仅 `~/.dragon-li/`、禁 shell、域名白名单框架）。

## Dependencies

- `desktop-runtime-core`
