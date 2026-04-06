<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import ChatPage from './pages/ChatPage.vue'
import MemoryPage from './pages/MemoryPage.vue'
import SettingsPage from './pages/SettingsPage.vue'
import { normalizePath, pathFromHash, routes } from './router/routes'
import { appState } from './state/appState'

const routeComponents = {
  '/chat': ChatPage,
  '/settings': SettingsPage,
  '/memory': MemoryPage
}

const currentPath = ref('/memory')

const currentRoute = computed(() => routes.find((route) => route.path === currentPath.value) ?? routes[2])
const currentView = computed(() => routeComponents[currentPath.value] ?? MemoryPage)

let unlistenMemoryUpdate = null

function syncRoute() {
  const path = normalizePath(pathFromHash(window.location.hash))
  currentPath.value = path
  appState.nav.lastVisitedPath = path
}

function go(path) {
  const target = normalizePath(path)
  if (target === currentPath.value) return
  window.location.hash = target
}

onMounted(async () => {
  if (!window.location.hash) {
    window.location.hash = appState.nav.lastVisitedPath
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
    <aside class="sidebar">
      <div class="brand">
        <h1>Dragon-Li</h1>
        <p>P0 Workspace</p>
      </div>
      <nav class="nav">
        <button
          v-for="route in routes"
          :key="route.path"
          :class="['nav-item', { active: currentPath === route.path }]"
          @click="go(route.path)"
        >
          <div class="nav-content">
            <span>{{ route.label }}</span>
            <span 
              v-if="route.path === '/memory' && appState.memory.unreviewedCount > 0" 
              class="badge"
            >
              {{ appState.memory.unreviewedCount > 99 ? '99+' : appState.memory.unreviewedCount }}
            </span>
          </div>
        </button>
      </nav>
      <p class="hint">当前页面：{{ currentRoute.label }}</p>
    </aside>

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
  display: grid;
  grid-template-columns: 250px minmax(0, 1fr);
  background:
    radial-gradient(circle at 100% 0%, #efe4d6, transparent 36%),
    radial-gradient(circle at 0% 100%, #e8dccb, transparent 33%),
    #f4efe6;
  color: #222226;
  font-family: 'Avenir Next', 'Segoe UI', sans-serif;
}

.sidebar {
  padding: 1rem;
  border-right: 1px solid #d4c8b8;
  background: rgba(255, 251, 244, 0.92);
  backdrop-filter: blur(6px);
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.brand h1 {
  margin: 0;
  font-size: 1.2rem;
}

.brand p {
  margin: 0.2rem 0 0;
  color: #6f6561;
  font-size: 0.82rem;
}

.nav {
  display: grid;
  gap: 0.45rem;
}

.nav-item {
  border: 1px solid #cdbfac;
  background: #f5eee4;
  color: #2f2b28;
  border-radius: 10px;
  padding: 0.6rem 0.7rem;
  text-align: left;
  font: inherit;
  cursor: pointer;
}

.nav-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.badge {
  background-color: #e63946;
  color: white;
  font-size: 0.75rem;
  font-weight: 600;
  padding: 0.1rem 0.4rem;
  border-radius: 12px;
  line-height: 1;
}

.nav-item.active {
  background: #2d6a4f;
  border-color: #2d6a4f;
  color: #fff;
}

.hint {
  margin-top: auto;
  color: #746a62;
  font-size: 0.8rem;
}

.content {
  padding: 1rem;
  overflow: auto;
  height: 100%;
}

@media (max-width: 920px) {
  .app-shell {
    grid-template-columns: 1fr;
  }

  .sidebar {
    border-right: 0;
    border-bottom: 1px solid #d4c8b8;
  }
}
</style>
