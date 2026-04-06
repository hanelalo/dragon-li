# Proposal: chat-provider-core

## Why

Provider 适配与对话编排耦合紧密，应作为一个完整变更落地。

## What

- 支持 OpenAI 与 Anthropic。
- 实现统一 chat 调用与流式事件。
- 落地错误映射与重试策略（429 不自动重试）。

## Dependencies

- `desktop-runtime-core`
- `config-and-guardrails-core`
- `sqlite-conversation-core`
