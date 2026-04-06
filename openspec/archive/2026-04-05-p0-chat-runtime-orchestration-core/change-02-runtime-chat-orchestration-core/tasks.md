# Tasks: change-02-runtime-chat-orchestration-core

- [x] 统一并冻结 `chat_send` 请求/响应字段定义。
- [x] 整理 prompt 分层构建逻辑并补齐输入校验。
- [x] 固化流式事件协议（delta/done）与异常中止语义。
- [x] 补齐成功/失败路径的 request_log 写入一致性。
- [x] 接入 memory 注入步骤并设定注入上限。
- [x] 增加最小集集成验证（成功、超时、鉴权失败、配置缺失）。

## 验收清单

- [x] 聊天编排接口可被前端直接接入。
- [x] 错误码与重试行为符合文档约定。
- [x] 任意失败请求可通过 `request_id` 追踪。
