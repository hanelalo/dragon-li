# Tasks: change-06-chat-provider-structured-output

- [ ] 扩展 `ChatAdapter` Trait 增加非流式和结构化输出的方法声明。
- [ ] 实现 `OpenAIAdapter` 针对 JSON 结构化的 `build_json_request` 与 `parse_json_response`。
- [ ] 实现 `AnthropicAdapter` 针对 JSON 结构化的 `build_json_request` 与 `parse_json_response`（利用 Prefill）。
- [ ] 在 `ChatService` 封装 `chat_with_retry_json` 重试调用层。
- [ ] 编写一个端到端的 Rust 单元测试，向 OpenAI 和 Anthropic 发起一段闲聊，验证返回的 JSON 结构体。

## 验收清单
- [ ] 可以通过 `chat_with_retry_json` 返回一个反序列化成功的强类型 Rust Struct。
- [ ] 模型如果输出非 JSON 格式或报错，能正确转为 `ChatError::ParseFailed`。