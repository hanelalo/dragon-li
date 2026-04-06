# change-06-chat-provider-structured-output

## Status
- **Completed on**: 2026-04-06
- **Component**: `chat_provider.rs`

## Summary
实现了大模型非流式结构化输出的支持：
1. 扩展了 `ChatAdapter` Trait，新增 `build_json_request` 与 `parse_json_response` 接口。
2. 实现了对 OpenAI 和 Anthropic 的兼容（分别处理了 `response_format` 和 Prefill 的解析拼装）。
3. 增加了 Markdown Strip 逻辑，清理大模型习惯性带有的 ````json` 包装。
4. 在 `ChatService` 层封装了带有网络重试和反序列化的 `chat_with_retry_json` 方法。
5. 补充了相关的反序列化容错单元测试，测试全部通过。

为后续的后台自动记忆提取和结构化数据生成提供了稳定的基础能力。