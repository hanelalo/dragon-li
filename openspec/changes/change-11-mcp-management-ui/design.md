# Design: change-11-mcp-management-ui

## 1. 数据模型设计 (SQLite)
在 `08-sqlite-ddl.md` 基础上新增表 `mcp_servers`：
```sql
CREATE TABLE IF NOT EXISTS mcp_servers (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  transport_type TEXT NOT NULL CHECK (transport_type IN ('stdio', 'sse', 'streamable_http')),
  
  -- stdio 独有
  command TEXT,
  args_json TEXT,
  env_json TEXT,
  
  -- http/sse 共用
  url TEXT,
  headers_json TEXT,
  
  status TEXT NOT NULL CHECK (status IN ('connected', 'error', 'disconnected', 'disabled')),
  enabled INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

## 2. 自动自举与生命周期 (Auto-bootstrap)
- **应用启动**：在 Tauri 的 `setup` 阶段，从 SQLite 读取所有 `enabled=1` 的 MCP Server 记录。
- **后台连接**：派发后台异步任务（`tokio::spawn`）自动拉起这些 Server 并执行 `initialize` 握手。
- **状态驻留**：握手成功后，将其 `McpClient` 实例存入全局状态 `AppState` 中的 `McpConnectionManager`，供后续对话流直接使用。

## 3. Tauri IPC 通信
新增以下 Commands：
- `mcp_server_create`
- `mcp_server_update`
- `mcp_server_delete`
- `mcp_server_list`: 返回所有配置及其当前内存状态（包括 `connecting`、`connected`、`error` 等实时状态）。
- `mcp_server_connect`: 前端触发手动连接/重连，测试连通性，更新 `AppState` 中的实例。
- `mcp_server_get_tools`: 连通成功后，从 `AppState` 的缓存或发起请求拉取该 Server 提供的 JSON Schema 列表（Tools）。

## 4. UI 交互设计
在 `SettingsPage.vue`（或聚合配置页）左侧侧边栏新增 `MCP Servers` 标签。
- 列表页：显示所有配置的 Server 及其运行状态红绿灯（包含加载中的 Spinner 和连通后的绿点）。
- 详情页：
  - Type 下拉选择（stdio, sse, streamable_http）。
  - 动态表单：如果选 stdio，展示 Command, Args(数组), Env(Key-Value 表单)；如果选 http，展示 URL 和 Header (Key-Value 表单)。
  - "Connect / Test" 按钮。
  - 成功连接后，在表单下方渲染一个只读的卡片列表，展示获取到的 `Tools` (包含 name, description)。
