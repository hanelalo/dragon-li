# Tasks: change-11-mcp-management-ui

- [ ] 在 `sqlite_store.rs` 补充 `mcp_servers` 的 DDL 建表语句与 CRUD 方法。
- [ ] 注册相关的 Tauri Commands (List, Create, Update, Delete)。
- [ ] 在 `AppState` 中维护一份活跃的 `McpClient` 字典（根据配置的 enabled 状态自动拉起连接）。
- [ ] 提供 `mcp_server_connect` 和 `mcp_server_get_tools` 接口，与后端的活跃连接通讯。
- [ ] 修改前端 Settings 页面结构，增加 MCP Server 列表侧边栏。
- [ ] 编写前端的动态配置表单，支持 Stdio 和 HTTP(s) 参数。
- [ ] 编写状态连通性测试逻辑与 UI 上的状态指示灯。
- [ ] 在详情页底端展示拉取到的 Tools 列表。

## 验收清单
- [ ] 可以在前端成功配置一个远端/本地的 MCP Server。
- [ ] 点击连接后，指示灯变为 connected，且能正确展示出 Tools 的名称与描述。
- [ ] SQLite 数据可正常落盘，重启应用后配置依然存在。
