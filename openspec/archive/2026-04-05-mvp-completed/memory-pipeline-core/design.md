# Design: memory-pipeline-core

- 候选状态：pending/approved/rejected/conflicted。
- 审核通过后：写 Markdown -> 更新索引表 -> 记录日志。
- 删除/恢复记忆时同步维护 postings（memory_index_terms）。
