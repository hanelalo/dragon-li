# Design: change-10-mcp-core-and-transport

## 核心抽象与 Trait (完全异步架构)
在 Rust 后端新建一个模块 `mcp/`，由于 MCP 的网络与本地进程通信是重 I/O 操作，必须基于 `tokio` 异步运行时构建。

### 1. Transport Trait
```rust
#[async_trait::async_trait]
pub trait McpTransport: Send + Sync {
    async fn start(&mut self) -> Result<(), McpError>;
    async fn send_message(&self, msg: JsonRpcMessage) -> Result<(), McpError>;
    async fn receive_message(&mut self) -> Result<JsonRpcMessage, McpError>;
    async fn close(&mut self) -> Result<(), McpError>;
}
```

### 2. 协议实现
- **`StdioTransport`**: 内部使用 `tokio::process::Command`，配置 `stdin` 和 `stdout` 为 `Stdio::piped()`，拉起外部命令并循环读取流。支持注入环境变量（env）。引入 `tokio_util::codec::LinesCodec` 处理基于换行符的 JSON-RPC 帧解析。
- **`SseTransport`**: 使用异步 HTTP 客户端（如 `reqwest`），连接 `/sse` 端点建立长连接监听事件流（Event Stream），发送消息则异步调用 `/message`（或类似端点），支持 Header 注入鉴权。
- **`StreamableHttpTransport`**: 发起单个异步 HTTP 请求并获取长连接的 Stream，兼顾双向数据流。

### 3. Client 状态机与并发管理
定义 `McpClient` 结构体，内部持有 `McpTransport`，并通过 `tokio::sync::mpsc` 或 `broadcast` 管理消息路由。维护以下状态：
- `Connecting`: 正在进行握手（异步等待）。
- `Connected`: 握手成功（发送 `initialize` 并收到 `initialized`）。
- `Disconnected`: 主动断开或远端关闭。
- `Error`: 连接崩溃或遇到严重异常。

**并发调用路由**：
由于同一个 `McpClient` 可能被并发调用多个 Tool，内部需维护 `HashMap<String, tokio::sync::oneshot::Sender<JsonRpcMessage>>`，通过 JSON-RPC 的 `id` 字段将收到的响应正确路由给等待的调用者。

### 4. 协议载荷
使用 `serde` 强类型定义 JSON-RPC 2.0 请求与响应：
- `InitializeRequest` / `InitializeResult`
- `ToolsListRequest` / `ToolsListResult`
- `ToolsCallRequest` / `ToolsCallResult`
