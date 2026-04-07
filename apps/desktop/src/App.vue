<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import ChatPage from './pages/ChatPage.vue'
import MemoryPage from './pages/MemoryPage.vue'
import SettingsPage from './pages/SettingsPage.vue'
import McpPage from './pages/McpPage.vue'
import { normalizePath, pathFromHash } from './router/routes'
import { appState } from './state/appState'

const routeComponents = {
  '/chat': ChatPage,
  '/settings': SettingsPage,
  '/memory': MemoryPage,
  '/mcp': McpPage
}

const currentPath = ref('/chat')

const currentView = computed(() => routeComponents[currentPath.value] ?? ChatPage)

let unlistenMemoryUpdate = null

function syncRoute() {
  const path = normalizePath(pathFromHash(window.location.hash))
  currentPath.value = path
  appState.nav.lastVisitedPath = path
}

onMounted(async () => {
  if (!window.location.hash) {
    window.location.hash = appState.nav.lastVisitedPath || '/chat'
  }
  syncRoute()
  window.addEventListener('hashchange', syncRoute)

  // Fetch initial unreviewed memory count
  try {
    const res = await invoke('memory_count_pending')
    appState.memory.unreviewedCount = res.data.count
  } catch (err) {
    console.error('Failed to get initial memory count:', err)
  }

  // Listen for auto-extraction updates from backend
  listen('memory_candidates_updated', (event) => {
    if (event.payload && typeof event.payload.unreviewed_count === 'number') {
      appState.memory.unreviewedCount = event.payload.unreviewed_count
      
      // If we are currently on the MemoryPage, we should reload the candidates
      // Note: We dispatch a custom event that MemoryPage can listen to
      window.dispatchEvent(new CustomEvent('memory-candidates-refreshed'))
    }
  }).then((unlisten) => {
    unlistenMemoryUpdate = unlisten
  }).catch((err) => {
    console.error('Failed to listen memory updates:', err)
  })
})

onBeforeUnmount(() => {
  window.removeEventListener('hashchange', syncRoute)
  if (typeof unlistenMemoryUpdate === 'function') {
    unlistenMemoryUpdate()
  }
})
</script>

<template>
  <div class="app-shell">
    <main class="content">
      <KeepAlive>
        <component :is="currentView" />
      </KeepAlive>
    </main>
  </div>
</template>

<style scoped>
.app-shell {
  height: 100%;
  width: 100%;
  display: flex;
  background:
    radial-gradient(circle at 100% 0%, #efe4d6, transparent 36%),
    radial-gradient(circle at 0% 100%, #e8dccb, transparent 33%),
    #f4efe6;
  color: #222226;
  font-family: 'Avenir Next', 'Segoe UI', sans-serif;
}

.content {
  flex: 1;
  padding: 0;
  overflow: auto;
  height: 100%;
  min-width: 0;
}
</style>
