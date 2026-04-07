# Tasks: change-10-mcp-core-and-transport

- [ ] 在 `src-tauri/src` 下新建 `mcp` 模块。
- [ ] 定义 `McpError`、`JsonRpcMessage` 和 `McpTransport` Trait。
- [ ] 实现 `StdioTransport`，支持传入 command、args、env。
- [ ] 实现 `SseTransport`，支持解析 SSE stream 与 Header 注入鉴权。
- [ ] 实现 `StreamableHttpTransport`，支持官方最新单端点长连接通信与鉴权。
- [ ] 编写 `McpClient`，完成 `initialize` 握手与 `tools/list` 协议。
- [ ] 编写基础的单元测试，模拟三种传输方式进行协议解析和收发测试。

## 验收清单
- [ ] 能够在 Rust 测试中成功拉起一个本地的 Dummy Python/Node MCP Server（stdio），并完成握手。
- [ ] JSON-RPC 的序列化与反序列化无异常。
