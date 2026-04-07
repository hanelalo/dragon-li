# Proposal: change-10-mcp-core-and-transport

## Why
为了让 Agent 具备真正的本地与外部工具调用能力，我们需要接入标准 MCP (Model Context Protocol)。作为基建的第一步，我们需要在 Rust 核心层实现 MCP Client 的通信协议抽象与基础握手逻辑。这是整个 Phase 2 的核心基础。

## What
- 支持三种 Transport 协议：
  1. `stdio`（标准输入输出，用于拉起本地子进程）。
  2. `sse`（Server-Sent Events，兼容早期/部分现存 HTTP Server）。
  3. `streamable http`（官方推荐最新 HTTP 协议）。
- 预留 MCP Primitives 抽象（Tools, Resources, Prompts），但本期业务逻辑仅聚焦打通 **Tools** 的底层获取（`tools/list`）和调用（`tools/call`）。
- 实现基于 JSON-RPC 2.0 的消息封装。

## Dependencies
- MVP (Phase 1 基础架构)
