# Design: change-08-memory-notification-ui

## 1. 后端发送 Event
在 `main.rs` 的提取异步任务 (`tauri::async_runtime::spawn`) 末尾，当确认有至少一条新记录成功通过 `MemoryPipeline` 写入 `memory_candidates` 时，由于当前闭包内已经持有 `app_handle`，可以直接通过 `app_clone.emit` 向前端下发通知：
```rust
let unreviewed_count = // count query from sqlite...
app_clone.emit("memory_candidates_updated", json!({
    "unreviewed_count": unreviewed_count,
    "new_memories": inserted_count
}))?;
```

## 2. 前端状态存储与审核联动
在 `src/main.js` 或一个类似 `composables/useMemoryStore.js` 的地方：
```javascript
import { ref } from 'vue'
import { listen } from '@tauri-apps/api/event'

export const unreviewedMemoryCount = ref(0)

export function setupMemoryListeners() {
  listen('memory_candidates_updated', (event) => {
    unreviewedMemoryCount.value = event.payload.unreviewed_count
    // Optional: trigger a toast notification here
  })
}

// 审核成功后递减
export function decrementUnreviewedCount() {
  if (unreviewedMemoryCount.value > 0) {
    unreviewedMemoryCount.value--
  }
}
```
前端在 Memory Center 调用 `memory_review_candidate` (Approve/Reject) 成功后，手动调用 `decrementUnreviewedCount` 更新角标，无需重新请求后端。

## 3. UI 交互组件 (Badge)
在 `src/components/layout/AppSidebar.vue`（或侧边栏相关的组件）中：
```vue
<template>
  <nav>
    <router-link to="/chat">Chat</router-link>
    <router-link to="/memory">
      Memory
      <span v-if="unreviewedMemoryCount > 0" class="badge">
        {{ unreviewedMemoryCount }}
      </span>
    </router-link>
  </nav>
</template>
```

## 4. 初始化加载
当应用刚打开时（`onMounted`），前端主动向后端发一个请求获取当前的待审核记忆数量，以便刚启动就有红点提示。