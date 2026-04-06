# Design: change-06-chat-provider-structured-output

## 1. 扩展 ChatAdapter
修改 `src-tauri/src/chat_provider.rs` 中的 `ChatAdapter`：
```rust
pub trait ChatAdapter: Send + Sync {
    // 原有的流式接口
    fn build_stream_request(&self, req: &ChatRequestInput, profile: &ApiProfile, model: &str) -> HttpRequest;
    fn parse_stream_line(&self, line: &str) -> Result<Vec<ChatStreamEvent>, ChatError>;
    
    // 新增非流式 JSON 接口
    fn build_json_request(&self, req: &ChatRequestInput, profile: &ApiProfile, model: &str) -> HttpRequest;
    fn parse_json_response(&self, response_body: &str) -> Result<serde_json::Value, ChatError>;
}
```

## 2. JSON 解析容错机制 (Markdown Stripping)
很多大模型即使在 JSON 模式下，也会习惯性地输出 ```json ... ``` 的 Markdown 格式包装。
在各 `ChatAdapter` 的 `parse_json_response` 内部实现中，需要增加一步**正则或字符串截取清理**，把首尾的 ```json 和 ``` 剔除掉，再交给 `serde_json::from_str` 解析返回 `Value`。这样可以保证 Service 层拿到的直接是合法的 JSON 对象。

## 3. OpenAI 实现
- 设置 `"stream": false`。
- 如果支持，传入 `"response_format": { "type": "json_object" }` 强制要求输出 JSON。
- 解析返回体 `choices[0].message.content` 为 JSON。

## 4. Anthropic 实现
- 设置 `"stream": false`。
- Anthropic 默认没有 `response_format` 字段，可以通过在 Assistant 的 `messages` 数组末尾预填入 `{"role": "assistant", "content": "{"}` 的方式（Prefill），利用补全机制强制它以 JSON 格式回复。
- **注意**：由于大模型的回复是从预填的 `{` 之后开始的，因此在 `parse_json_response` 解析前，必须**手动在 `response_body` 头部拼接上 `{`**，否则会报 JSON 解析错误。

## 5. 调用封装
在 `ChatService` 中新增 `chat_with_retry_json<T: DeserializeOwned>(...) -> Result<T, ChatError>`，用来处理网络重试和自动反序列化。