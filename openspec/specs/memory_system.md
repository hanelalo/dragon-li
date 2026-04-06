# Memory System Specification

**Status**: Active (MVP Phase)
**Last Updated**: 2026-04-06

## 1. Overview
Memory System 是 Dragon Li 的核心特色之一，提供了一套长期记忆提取、审核、本地索引与自动注入的流水线。该系统无需任何外部服务（如向量数据库），以纯本地优先（Local-First）的方式实现。

## 2. Memory Pipeline Lifecycle

### 2.1 Extraction (Heuristic Rules - MVP)
- **触发机制**: 当前为用户手动在 Memory Center 点击 `Extract Candidates` 按钮触发。
- **算法流程 (`memory_pipeline.rs`)**:
  - `Sentence Splitting`: 根据标点符号将用户文本拆分为短句。
  - `Keyword Matching`: 扫描每个句子中的关键词（如“喜欢”、“必须”、“项目”等）。
  - `Classification & Confidence`: 为命中关键词的句子分配类型（`preference`, `constraint`, `project`, `task`, `fact`）及一个基础的置信度分数（Confidence）。
  - `Tag Extraction`: 提取 2-20 个字符的中英文字词作为候选标签（Tags）。
- **状态**: 提取后的数据以 `pending`（待审核）状态保存在 `memory_candidates` 表中。

### 2.2 Review (Approve/Reject)
- 用户在 Memory Center 中可以点击 `pending` 卡片进行审核。
- **Approve**: 
  - 状态变为 `approved`。
  - 在文件系统 `~/.dragon-li/memory/long_term/` 下生成一个实体 Markdown 文件，包含 `summary`, `tags`, `type`, `evidence` 等元数据。
  - 同步调用 `upsert_index_tx` 将数据写入倒排索引表。
- **Reject**:
  - 状态变为 `rejected`。
  - 在数据库中标记更新，不生成实体文件，前端列表清空当前焦点。

### 2.3 Local Indexing (TF-IDF)
- 索引系统由三个 SQLite 表组成：
  - `memory_index_docs`: 记忆主文档表。
  - `memory_index_terms`: **倒排词项表（核心）**，保存词频（TF, Term Frequency）及字段权重（Weight）。
  - `memory_index_stats`: 文档频率表，用于后续的 IDF 计算（MVP 阶段暂作保留）。
- **Tokenization**: 极简分词器。将 `summary`, `tags`, `type`, `evidence` 字段切分成词项。不同字段赋予不同权重（如 Tags=1.5, Summary=1.3, Type=1.0, Evidence=0.8）。

### 2.4 Context Injection (Retrieval)
- **触发机制**: 用户在 Chat 页面发送消息时（`chat_send`），触发 `inject_memory_context`。
- **查询过程 (`query_index`)**:
  - 将用户的查询（Query）分词为多个 Terms。
  - 在 `memory_index_terms` 表中匹配，通过公式 `SUM(t.tf * t.weight)` 累加得分（Score）。
  - 取出得分最高的 Top 3（可通过配置调整）关联记忆文档。
- **注入 Prompt**: 将召回的记忆合并为 Markdown 字符串，作为 System Prompt 或 User Context 的一部分，静默发送给大模型。

## 3. UI Interactions
- **Memory Center**:
  - 列表展示与分页（按时间倒序）。
  - **后端过滤**: 通过 `Session ID`, `Type`, `Status`, `Min Confidence`, `Tags` 在 SQLite 层面进行过滤。
  - **前端过滤**: 通过 `Search keyword` 在 Vue 内存中进行快速字符匹配搜索。
  - 版本历史（Versions）回溯机制（基于文件系统备份）。