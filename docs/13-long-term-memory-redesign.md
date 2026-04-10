# 长期记忆系统完整改造方案

## 一、设计目标

- **真正的自动记忆**：提取 + 合并 + 淘汰全自动，无需人工审核
- **低成本高效率**：每 3 轮提取一次，整合按阈值/事件触发，控制 LLM 调用
- **高质量检索**：jieba-rs 中文分词、IDF 评分、时间衰减
- **数据正确性**：修复已知 bug，状态流转清晰
- **可扩展**：Indexer trait 预留向量检索升级路径

## 二、整体架构：两阶段管道

### 当前流程（废弃）

```
每轮对话 → 完整历史 + 最新消息 → LLM 提取 → pending 候选 → 等待人工审核
```

### 改造后流程

```
┌──────────────────────────────────────────────────────────┐
│                      用户对话                              │
└────────────────────────┬─────────────────────────────────┘
                         │
                         ▼
                ┌─── 每 3 轮触发 ───┐
                │  阶段一：提取（轻量）│
                │                    │
                │  输入：             │
                │   提取源：最近 3 轮  │
                │   去重参考：         │
                │     running summary │
                │     更早 3 轮对话   │
                │                    │
                │  输出：0~N 条候选   │
                │  status='candidate'│
                └────────┬───────────┘
                         │
                         ▼
                ┌─── 候选积累中 ───┐
                │                  │
                │  检查触发条件：    │
                │  a) 候选 >= 5 条？│
                │  b) 会话结束？     │
                │  c) 下次启动？     │
                │  → 取先到者       │
                └────────┬─────────┘
                         │ 触发
                         ▼
                ┌── 阶段二：整合（重量）──┐
                │                        │
                │  对每条候选：            │
                │   TF-IDF 检索 top-5    │
                │   相似已有记忆          │
                │   LLM 做对比决策        │
                │                        │
                │  输出决策：              │
                │   ADD / UPDATE         │
                │   DELETE / NOOP        │
                └────────┬───────────────┘
                         │
                         ▼
                ┌── 阶段三：自动执行 ──┐
                │                     │
                │  ADD → active       │
                │       写 Markdown   │
                │       建索引        │
                │                     │
                │  UPDATE → 更新目标  │
                │          写新版本    │
                │          重建索引    │
                │                     │
                │  DELETE → superseded │
                │          保留历史    │
                │                     │
                │  NOOP → rejected    │
                └─────────────────────┘
                         │
                         ▼
                ┌── 整合完成后 ──┐
                │ 重新生成       │
                │ running summary│
                └───────────────┘
```

## 三、Running Summary

新增概念，贯穿整个记忆系统的去重和上下文理解。

- **是什么**：将所有 active 记忆压缩为一段 ~200 token 的精炼文本
- **何时更新**：每次整合执行完成后重新生成
- **用途**：
  - 提取阶段：作为去重上下文，避免 LLM 提取已有信息
  - 让提取 prompt 不必依赖过多对话历史
- **存储**：DB 单行表 `memory_summary`，应用启动时加载到内存

## 四、提取触发机制

### 4.1 提取触发频率

每 **3 轮对话**自动触发一次。

```
第 1 轮 → 不提取
第 2 轮 → 不提取
第 3 轮 → 提取最近 3 轮（第 1-3 轮）的对话内容
第 4 轮 → 不提取
第 5 轮 → 不提取
第 6 轮 → 提取最近 3 轮（第 4-6 轮）的对话内容
...
```

好处：
- API 调用量降低 2/3
- 3 轮对话提供更丰富的上下文，单轮提取容易遗漏跨轮的信息
- 闲聊/确认类连续多轮大概率被归为一次提取，LLM 判断为 `<none>` 就不写入

### 4.2 提取上下文窗口

LLM 的完整输入：

| 区域 | 内容 | 用途 |
|------|------|------|
| **提取源** | 最近 3 轮对话（6 条消息） | 只从这里提取新记忆 |
| **去重参考** | running summary + 更早 3 轮对话 | 仅供去重参考，禁止从中提取 |

预估成本：6 条消息 + summary + 3 条历史 ≈ 1000-1500 tokens/次，平均每轮约 350-500 tokens。

### 4.3 整合触发条件

| 触发条件 | 说明 |
|----------|------|
| 候选积累 >= 5 条 | 同一轮对话中自动触发 |
| 会话结束 | 切换会话、离开聊天页面、下次启动时补跑 |

## 五、会话结束判定

### 5.1 触发点

| 触发点 | 检测方式 | 说明 |
|--------|---------|------|
| 切换会话 | `watch(activeSessionId)` 变化 | 对旧 session 触发 session_end |
| 离开聊天页面 | `onDeactivated`（KeepAlive 钩子） | 对当前 session 触发 session_end |
| 下次启动 | app 启动初始化 | 检查未整合候选，补跑整合 + 重新生成 summary |

### 5.2 session_end 守卫逻辑

```
session_end(session_id):
  1. 查全局未整合候选数
     → 0 → 直接返回
  2. 该 session 距上次提取不满 3 轮？
     → 是 → 补跑一次提取
  3. 全局未整合候选 >= 5？
     → 是 → 触发整合 + 重新生成 summary
     → 否 → 直接返回（留给后续 session_end 或下次启动兜底）
```

### 5.3 反复横跳场景

用户在多个会话间来回切换不发消息：
- 每次切换触发 session_end → 查 DB count → 0 → 直接返回
- 成本仅为一次 DB count 查询，无多余 LLM 调用

### 5.4 直接关 App

- 不做任何收尾
- 下次启动时检查 `status='candidate'` 的记录，补跑整合 + 重新生成 summary

### 5.5 内存状态

- 维护 `HashMap<session_id, turn_count>` 记录每个 session 的已发送消息轮次
- 每次提取后重置对应 session 的计数器
- 存在内存中，启动时重置，不持久化

## 六、DB Schema 变更

### 6.1 `memory_candidates` 表

**status 约束改为：** `('candidate', 'active', 'superseded', 'rejected')`

| 新状态 | 含义 | 替代旧状态 |
|--------|------|-----------|
| `candidate` | 新提取，等待整合 | 替代 `pending` |
| `active` | 整合确认的有效记忆 | 替代 `approved` |
| `superseded` | 被新信息取代 | 替代 `conflicted` |
| `rejected` | 整合判断无效/重复 | 同旧 `rejected` |

**新增字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `superseded_by` | TEXT NULL | 被哪条记忆取代，形成追溯链 |

### 6.2 新增 `memory_summary` 表

```sql
CREATE TABLE IF NOT EXISTS memory_summary (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  summary TEXT NOT NULL DEFAULT '',
  updated_at TEXT NOT NULL
);
```

### 6.3 迁移策略

启动时检测 schema 版本，自动执行：

```
pending    → candidate
approved   → active
conflicted → superseded
```

## 七、各模块详细改动

### 7.1 Python Agent

#### 7.1.1 提取接口改造 `/v1/chat/memory/extract`

**请求模型变更：**

```python
class MemoryExtractRequest(BaseModel):
    profile_id: str
    model: Optional[str] = None
    session_id: str
    recent_turns: List[ChatMessageContext] = []   # 最近 3 轮（提取源）
    earlier_turns: List[ChatMessageContext] = []   # 更早 3 轮（去重上下文）
    running_summary: str = ""                      # 记忆摘要（去重上下文）
    cfg: Optional[ApiProfilesConfig] = None
    # 删除 user_text、assistant_text、history
```

**提取 Prompt 改造：**

参考 Mem0 的设计，明确区分"提取源"和"去重参考"：

```
你是一个记忆提取专家。

## 提取源（只从这里提取新记忆）
{recent_turns 的 6 条消息}

## 去重参考（禁止从中提取）
已有记忆摘要：
{running_summary}

更早对话：
{earlier_turns}

## 任务
从「提取源」中提取关于用户的持久性事实、偏好、约束、项目信息、任务。
如果「去重参考」中已包含相同或语义相似的信息，不要重复提取。
没有值得记忆的新信息时，返回 {"memories": []}

## 输出格式
{"memories": [{"type_": "fact|preference|constraint|project|task", "summary": "...", "evidence": "...", "tags": [...], "confidence": 0.0-1.0}]}
```

#### 7.1.2 新增整合接口 `/v1/memory/consolidate`

```python
class ConsolidationItem(BaseModel):
    candidate: ConsolidationCandidate
    similar_memories: List[ExistingMemory]

class ConsolidationRequest(BaseModel):
    profile_id: str
    model: Optional[str] = None
    items: List[ConsolidationItem]
    cfg: Optional[ApiProfilesConfig] = None

class ConsolidationResponse(BaseModel):
    decisions: List[ConsolidationDecision]

class ConsolidationDecision(BaseModel):
    candidate_id: str
    action: str           # "ADD" | "UPDATE" | "DELETE" | "NOOP"
    target_memory_id: Optional[str]
    merged_summary: Optional[str]
    reason: str
```

**整合 Prompt（参考 Mem0 update phase）：**

对每条候选记忆，和其 top-5 相似已有记忆一起交给 LLM：

```
你是一个记忆管理专家。比较「候选记忆」和「已有相似记忆」，决定操作类型。

优先级：NOOP > DELETE > UPDATE > ADD

## 候选记忆
ID: {candidate_id}
类型: {type_}
摘要: {summary}
证据: {evidence}

## 已有相似记忆
{similar_memories_formatted}

## 操作定义
- ADD: 全新信息，与任何已有记忆不冲突
- UPDATE: 补充/修正已有记忆，返回合并后的 summary
- DELETE: 候选记忆表明某条已有记忆已过时/错误，标记旧记忆为 superseded
- NOOP: 与已有记忆语义重复，无新信息

## 输出格式（JSON 数组）
[{ "candidate_id": "...", "action": "ADD|UPDATE|DELETE|NOOP", "target_memory_id": null|"...", "merged_summary": null|"...", "reason": "..." }]
```

#### 7.1.3 新增摘要生成接口 `/v1/memory/generate_summary`

```python
class SummaryRequest(BaseModel):
    profile_id: str
    model: Optional[str] = None
    memories: List[ExistingMemory]      # 所有 active 记忆
    cfg: Optional[ApiProfilesConfig] = None

class SummaryResponse(BaseModel):
    summary: str                        # ~200 token 精炼文本
```

### 7.2 Rust 端

#### 7.2.1 `memory_pipeline.rs`

**废弃的函数：**

| 函数 | 原因 |
|------|------|
| `extract_candidates()` | 本地关键词匹配质量差，废弃 |
| `review_candidate()` | 人工审核流程，废弃 |
| `count_pending_candidates()` | 无 pending 概念，废弃 |

**改造的函数：**

| 函数 | 改动 |
|------|------|
| `save_extracted_candidates()` | status 改为 `'candidate'`；去重从精确匹配改为与 active 记忆的 Jaccard 相似度 > 0.8 |
| `read_memory_doc()` | 删除静默改写状态的逻辑，返回实际状态 |
| `query_index()` | 加入 IDF 评分 + 时间衰减因子（见第八节） |
| `tokenize()` | 使用 `jieba-rs` 替换逐字拆分（见第九节） |
| `upsert_terms_tx()` | 配合 jieba 分词改动 |
| `upsert_index_tx()` | stats 改为增量更新（见第十节） |

**新增的函数：**

| 函数 | 说明 |
|------|------|
| `count_unconsolidated()` | 统计 `status='candidate'` 的数量 |
| `list_unconsolidated(limit)` | 获取未整合的候选 |
| `execute_consolidation(decisions)` | 执行 ADD/UPDATE/DELETE/NOOP 决策（见 7.2.4） |
| `read_summary()` / `save_summary(text)` | 读写 running summary |

#### 7.2.2 `execute_consolidation` 详细逻辑

```rust
pub fn execute_consolidation(
    &self,
    decisions: Vec<ConsolidationDecision>
) -> Result<usize, MemoryError>
```

对每条决策：

| action | 对候选 | 对目标记忆 | 额外操作 |
|--------|--------|-----------|---------|
| **ADD** | `status='active'`，写 Markdown，建索引 | 无 | — |
| **UPDATE** | `status='rejected'` | `summary=merged_summary`，写新版本 Markdown，重建索引 | — |
| **DELETE** | `status='rejected'` | `status='superseded'`，`superseded_by=candidate_id` | — |
| **NOOP** | `status='rejected'` | 无 | — |

返回实际执行的操作数。

#### 7.2.3 Indexer trait 预留

```rust
pub trait MemoryIndexer: Send + Sync {
    fn index(&self, memory: &MemoryDoc) -> Result<(), MemoryError>;
    fn remove(&self, memory_id: &str) -> Result<(), MemoryError>;
    fn search(&self, query: &str, opts: SearchOptions) -> Result<Vec<SearchHit>, MemoryError>;
}

pub struct SearchOptions {
    pub min_confidence: f64,
    pub limit: usize,
    pub decay_rate: f64,
}

pub struct TfIdfIndexer { /* 当前实现迁移到此 */ }
// 未来: pub struct VectorIndexer { ... }
```

#### 7.2.4 `commands/chat.rs`

**`inject_memory_context` 改造：**

- `MEMORY_INJECTION_TOP_N`：3 → **5**
- 注入格式丰富化：

```
## 相关记忆
- [preference] 用户偏好暗色主题和简洁的 UI 设计 (conf: 0.85)
- [fact] 用户是一名前端工程师，主要使用 Vue 和 TypeScript (conf: 0.92)
```

- 短消息（< 10 字）跳过注入
- `min_confidence` 可配置（默认 0.6）

**提取后台任务改造：**

```
每 3 轮对话 → turn_count + 1
  当 turn_count % 3 == 0 时：
    构建 MemoryExtractRequest {
      recent_turns: 最近 3 轮对话（从 DB 取）
      earlier_turns: 更早 3 轮对话（从 DB 取）
      running_summary: 从 memory_summary 表取
    }
    POST /v1/chat/memory/extract
    保存结果（status='candidate'）
    重置 turn_count
    检查是否触发整合
```

**新增 `run_consolidation`：**

```rust
async fn run_consolidation(state, profile_id, model, cfg) {
    // 1. 取所有未整合候选
    let candidates = pipeline.list_unconsolidated(20);

    // 2. 对每条候选检索相似已有记忆
    let items = candidates.map(|c| {
        let similar = pipeline.query_index(&c.summary, 0.3, 5);
        ConsolidationItem { candidate: c, similar_memories: similar }
    });

    // 3. 调 Python Agent 做决策
    let decisions = agent.post("/v1/memory/consolidate", items);

    // 4. 执行决策
    pipeline.execute_consolidation(decisions);

    // 5. 重新生成 running summary
    let all_active = pipeline.list_long_term(None, None, 0.0, &[], 1000);
    let new_summary = agent.post("/v1/memory/generate_summary", all_active);
    pipeline.save_summary(&new_summary.summary);
}
```

#### 7.2.5 `commands/memory.rs`

**删除的命令：**

| 命令 | 原因 |
|------|------|
| `memory_extract_candidates` | 废弃本地规则提取 |
| `memory_review_candidate` | 废弃人工审核 |
| `memory_count_pending` | 无 pending 概念 |

**新增的命令：**

| 命令 | 说明 |
|------|------|
| `memory_count_unconsolidated` | 未整合候选数量 |
| `memory_trigger_consolidation` | 手动触发整合（会话结束时自动触发，此命令作为补充） |
| `memory_list_history(memory_id)` | 查看记忆变更历史（Markdown 版本链 + superseded 追溯链） |
| `memory_get_summary` | 获取当前 running summary |

**改造的命令：**

- `memory_list_candidates` → 默认列 `status='candidate'`，供调试查看
- `memory_list_long_term` → 默认列 `status='active'`，支持过滤 `superseded`

### 7.3 前端

#### MemoryPage.vue — 从审核队列改为浏览管理中心

```
┌────────────────┬────────────────────┬──────────────────┐
│ 记忆列表        │ 记忆详情            │ 变更历史          │
│                │                    │                  │
│ 筛选：          │ Type: preference   │ v3 (2026-04-10)  │
│  搜索关键词     │ Summary: ...       │ v2 (2026-04-08)  │
│  type 下拉     │ Evidence: ...      │ v1 (2026-04-05)  │
│  confidence 滑块│ Tags: [...]       │                  │
│                │ Confidence: 0.85   │ 取代原因：        │
│ Tab 切换：      │ Created: ...       │ "用户从C++转向Rust"│
│  Active        │ Updated: ...       │                  │
│  Superseded    │                    │                  │
│                │ Markdown 渲染       │ [恢复此版本]      │
│ 操作：          │                    │                  │
│  编辑 / 删除    │                    │                  │
└────────────────┴────────────────────┴──────────────────┘
```

**去掉：**
- 审核队列（Approve/Reject/Merge 按钮）
- Candidate Review 左侧面板
- pending badge

**新增：**
- 记忆列表支持按 `active` / `superseded` 过滤
- 记忆详情显示完整的 type、summary、evidence、tags、confidence、时间
- 变更历史面板：展示 Markdown 版本链 + superseded 追溯链
- 手动恢复某条被 superseded 的记忆（恢复到 active 状态）
- 手动编辑记忆内容（触发新版本写入）
- 手动删除记忆

#### SessionSidebar.vue

- 去掉 memory badge（unreviewed count）
- 去掉 `memory_candidates_updated` 事件监听

#### ChatPage.vue

- `watch(activeSessionId)` 变化时，对旧 session 调用 `memory_session_end`
- `onDeactivated` 时，对当前 session 调用 `memory_session_end`

## 八、检索质量改进

### 8.1 评分公式

**当前：** `score = SUM(tf * weight)`

**改造后：** `score = SUM(tf * weight * idf) * decay(age)`

| 因素 | 公式 | 说明 |
|------|------|------|
| TF | 已有 | 词频 |
| weight | 已有 | 字段权重（summary 1.3, tags 1.5, evidence 0.8, type 1.0） |
| IDF | `log(N / (1 + doc_freq))` | **新增**，稀有词权重更高，使用 `memory_index_stats` 表 |
| decay | `0.999 ^ hours_since_update` | **新增**，越旧衰减越大 |

### 8.2 注入改进

- 注入数量：3 → **5**
- 注入格式丰富化：`[type] summary (conf: 0.85)`
- 短消息（< 10 字）跳过注入
- `min_confidence` 可配置（默认 0.6）

## 九、分词改进

### 9.1 引入 jieba-rs

使用 `jieba-rs` crate（纯 Rust 实现，891 stars）替换当前 `tokenize()` 函数中的逐字拆分。

```toml
[dependencies]
jieba-rs = { version = "0.8", features = ["tfidf"] }
```

### 9.2 效果对比

| 输入 | 当前输出 | jieba-rs 输出 |
|------|---------|--------------|
| `我喜欢简洁的代码` | `['我','喜','欢','简','洁','的','代','码']` | `['我喜欢','简洁','的','代码']` |
| `偏好暗色主题` | `['偏','好','暗','色','主','题']` | `['偏好','暗色','主题']` |
| `用 Rust 写系统` | `['用',' ','R','u','s','t',' ','写','系','统']` | `['用','Rust','写','系统']` |

jieba-rs 天然支持中英文混合文本，无需额外处理。

### 9.3 影响范围

- 索引构建（`upsert_terms_tx`）
- 检索查询（`query_index`）
- 标签提取（`extract_tags`）
- 去重判断（Jaccard 相似度基于分词后的 token 集合）

## 十、数据正确性修复

| Bug | 修复方案 |
|-----|---------|
| 会话删除不级联索引 | `soft_delete_session` 级联更新 `memory_index_docs.deleted_at` |
| 会话恢复不恢复索引 | `restore_session` 重建对应记忆索引 |
| `read_memory_doc` 静默改写状态 | 删除该逻辑，返回实际状态 |
| stats 全量重建 | 改为增量更新：写入时 `INSERT OR REPLACE` 对应 term 的 doc_freq |

## 十一、性能优化

| 问题 | 方案 |
|------|------|
| 每次操作新建 DB 连接 | 引入 `r2d2-sqlite` 连接池 |
| 注入同步阻塞 | 用 `tokio::task::spawn_blocking` 包裹 |
| tag 过滤在内存中做 | 推到 SQL 层 |
| stats 全量重建 | 改为增量更新 |

## 十二、文件变动清单

### Rust

| 文件 | 改动量 | 说明 |
|------|--------|------|
| `memory_pipeline.rs` | 大改 | 废弃 3 函数、新增 4 函数、分词替换、检索改进 |
| `sqlite_store.rs` | 中改 | Schema 迁移、级联删除/恢复修复 |
| `commands/memory.rs` | 大改 | 删 3 命令、增 4 命令 |
| `commands/chat.rs` | 中改 | 注入改造、提取改造、整合触发、session_end |
| `Cargo.toml` | 小改 | +jieba-rs、+连接池 |

### Python

| 文件 | 改动量 | 说明 |
|------|--------|------|
| `agent/api/chat.py` | 中改 | 提取接口参数变更、新增整合/摘要接口 |
| `agent/llm/provider.py` | 大改 | 提取逻辑改造、新增整合/摘要 LLM 调用 |
| `agent/core/models.py` | 中改 | 新增 Consolidation/Summary 模型、改造 ExtractRequest |
| `agent/core/prompts.py` | 中改 | 提取 prompt 改造、新增整合 prompt、摘要 prompt |

### 前端

| 文件 | 改动量 | 说明 |
|------|--------|------|
| `MemoryPage.vue` | 大改 | 审核队列 → 浏览管理中心 |
| `SessionSidebar.vue` | 小改 | 去 badge、去事件监听 |
| `ChatPage.vue` | 小改 | session_end 触发 |

## 十三、实施阶段

### Phase 1：基础修复 + 分词（1-2 天）

- 修复会话删除/恢复级联索引
- 修复 `read_memory_doc` 状态问题
- 引入 `jieba-rs` 替换分词
- stats 增量更新

### Phase 2：Schema 迁移 + 新状态（1 天）

- 修改 status 约束：`candidate / active / superseded / rejected`
- 数据迁移：`pending → candidate`，`approved → active`，`conflicted → superseded`
- 新增 `memory_summary` 表
- 新增 `superseded_by` 字段

### Phase 3：提取管道改造（1-2 天）

- Python：改造提取 prompt + 请求模型
- Rust：提取触发改为每 3 轮 + 传 recent_turns/earlier_turns/running_summary
- 实现 summary 读写

### Phase 4：整合管道（2 天）

- Python：新增整合接口 + prompt + 摘要生成接口
- Rust：新增候选缓存、整合触发逻辑、`execute_consolidation`、summary 重生成
- session_end 守卫逻辑

### Phase 5：前端改造（1-2 天）

- MemoryPage 改为浏览管理中心三栏布局
- 去掉审核相关 UI
- 变更历史面板
- ChatPage session_end 事件

### Phase 6：检索质量 + 注入改进（1 天）

- IDF 评分实现
- 时间衰减实现
- 注入格式丰富化
- Indexer trait 抽取

### Phase 7：性能优化（1 天）

- 连接池引入
- 异步注入
- SQL 层 tag 过滤

**总计：8-11 天**
