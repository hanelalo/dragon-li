# Design: change-07-memory-auto-extraction-core

## 1. 结构化 JSON Schema 定义
在 `memory_pipeline.rs` 定义用于大模型返回的结构体：
```rust
#[derive(Deserialize)]
struct AutoExtractedMemory {
    summary: String,
    type_: String, // preference, constraint, project, fact, task
    tags: Vec<String>,
    evidence: String,
    confidence: f64,
}

#[derive(Deserialize)]
struct AutoExtractionResult {
    memories: Vec<AutoExtractedMemory>
}
```

## 2. 增量提取逻辑 (Delta Context)
- **取数逻辑**：如果每次聊天都发全量记录去提取，不仅消耗 Token 巨大，还会导致同一段记忆被重复提取。
- **改进策略**：在 `main.rs` 异步任务中，通过 `SqliteStore` 获取该 session 最新的一对对话（最后一条 User 消息 + 对应的 Assistant 回复）来组装 `ChatRequestInput`。
- **Prompt 引导**：在 Prompt 中明确说明：`"Your task is to extract ONLY NEW facts, preferences, or constraints revealed in the user's LATEST message... If no new information is present, return an empty array."`
- **调用模型**：在 `main.rs` 的异步闭包中直接调用 `ChatService::chat_with_retry_json` 发起提取请求，拿到 `AutoExtractionResult`。
- **落库**：在 `MemoryPipeline` 新增一个接收 `AutoExtractionResult`（或类似的强类型）和 `session_id` 的同步方法 `save_extracted_candidates`，将数据转为 `MemoryCandidateRecord` 并插入 `memory_candidates` 表。

## 3. 触发机制
在 `main.rs` 的 `chat_send` 中，成功将消息落库后：
```rust
// tauri::async_runtime::spawn 一个后台任务
let app_clone = app.clone();
let session_id_clone = request.session_id.clone();
let state_clone = state.inner().clone();
tauri::async_runtime::spawn(async move {
    // 1. 从 state_clone.sqlite_store 获取最后 2 条消息
    // 2. 组装 ChatRequestInput
    // 3. 调用 service.chat_with_retry_json 拿到提取结果
    // 4. 调用 state_clone.memory_pipeline.save_extracted_candidates 落库
    // 5. (关联 change-08) 通过 app_clone.emit 发送通知
});
```

## 4. 去重策略（MVP阶段简单实现）
- 如果我们每次只提取增量（Delta Context），大部分重复问题都能解决。
- 但为了防止极其相似的表达再次被提取，在插入前可以用 SQL 的 `LIKE` 语句在当前 Session 历史中检查是否已有高度相似的 `summary`。
- 在人工审核阶段，用户也可以选择拒绝。