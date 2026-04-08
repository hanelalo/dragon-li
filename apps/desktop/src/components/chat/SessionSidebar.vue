<script setup>
import { ref, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { appState } from '../../state/appState'

const props = defineProps({
  sessions: {
    type: Array,
    required: true
  },
  activeId: {
    type: String,
    required: true
  }
})

const emit = defineEmits(['select', 'create', 'rename', 'delete'])

const editingId = ref(null)
const editTitle = ref('')
const editInput = ref(null)
const showSettingsMenu = ref(false)
const currentHash = ref(window.location.hash)

function startRename(session) {
  editingId.value = session.id
  editTitle.value = session.title
  nextTick(() => {
    if (editInput.value && editInput.value[0]) {
      editInput.value[0].focus()
      editInput.value[0].select()
    }
  })
}

function submitRename(session) {
  if (!editTitle.value.trim() || editTitle.value.trim() === session.title) {
    editingId.value = null
    return
  }
  emit('rename', { id: session.id, title: editTitle.value.trim() })
  editingId.value = null
}

function handleKeydown(e, session) {
  if (e.key === 'Enter') {
    submitRename(session)
  } else if (e.key === 'Escape') {
    editingId.value = null
  }
}

function toggleSettingsMenu(event) {
  event.stopPropagation()
  showSettingsMenu.value = !showSettingsMenu.value
}

function closeMenu(e) {
  if (showSettingsMenu.value && !e.target.closest('.settings-wrapper')) {
    showSettingsMenu.value = false
  }
}

function goTo(path) {
  showSettingsMenu.value = false
  window.location.hash = path
  currentHash.value = `#${path}`
}

function handleHashChange() {
  currentHash.value = window.location.hash
}

onMounted(() => {
  window.addEventListener('click', closeMenu)
  window.addEventListener('hashchange', handleHashChange)
})

onBeforeUnmount(() => {
  window.removeEventListener('click', closeMenu)
  window.removeEventListener('hashchange', handleHashChange)
})
</script>

<template>
  <aside class="session-sidebar">
    <div class="mcp-nav-section">
      <button class="mcp-nav-btn" :class="{ active: currentHash === '#/mcp' }" @click="goTo('/mcp')">
        <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
          <line x1="9" y1="3" x2="9" y2="21"></line>
          <path d="M13 8l4 4-4 4"></path>
        </svg>
        <span>MCP</span>
      </button>
    </div>

    <div class="nav-divider"></div>

    <header>
      <h3>Chats</h3>
      <button class="icon-btn new-chat-btn" @click="emit('create')" title="New Chat">
        <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
          <line x1="12" y1="5" x2="12" y2="19"></line>
          <line x1="5" y1="12" x2="19" y2="12"></line>
        </svg>
      </button>
    </header>
    <ul class="session-list">
      <li
        v-for="session in sessions"
        :key="session.id"
        :class="{ active: session.id === activeId }"
        @click="emit('select', session.id)"
      >
        <div v-if="editingId === session.id" class="edit-mode">
          <input
            v-model="editTitle"
            @blur="submitRename(session)"
            @keydown="(e) => handleKeydown(e, session)"
            ref="editInput"
            autofocus
          />
        </div>
        <div v-else class="view-mode">
          <span class="title">{{ session.title }}</span>
          <div class="actions">
            <button class="icon-btn action-btn" @click.stop="startRename(session)" title="Rename">
              <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 20h9"></path>
                <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"></path>
              </svg>
            </button>
            <button class="icon-btn action-btn delete-btn" @click.stop="emit('delete', session.id)" title="Archive/Delete">
              <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="3 6 5 6 21 6"></polyline>
                <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
              </svg>
            </button>
          </div>
        </div>
      </li>
      <li v-if="sessions.length === 0" class="empty">
        No sessions
      </li>
    </ul>
    
    <footer class="sidebar-footer">
      <div class="settings-wrapper" @click.stop>
        <button class="footer-btn settings-btn" @click="toggleSettingsMenu" title="设置">
          <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="3"></circle>
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
          </svg>
          <span v-if="appState.memory.unreviewedCount > 0" class="badge menu-badge">
            {{ appState.memory.unreviewedCount > 99 ? '99+' : appState.memory.unreviewedCount }}
          </span>
        </button>

        <div v-if="showSettingsMenu" class="settings-menu">
          <button class="menu-item" @click="goTo('/settings')">
            <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
              <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
              <line x1="8" y1="21" x2="16" y2="21"></line>
              <line x1="12" y1="17" x2="12" y2="21"></line>
            </svg>
            <span>配置</span>
          </button>
          <button class="menu-item" @click="goTo('/memory')">
            <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path>
              <polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline>
              <line x1="12" y1="22.08" x2="12" y2="12"></line>
            </svg>
            <span>记忆</span>
            <span v-if="appState.memory.unreviewedCount > 0" class="badge">
              {{ appState.memory.unreviewedCount > 99 ? '99+' : appState.memory.unreviewedCount }}
            </span>
          </button>
        </div>
      </div>
    </footer>
  </aside>
</template>

<style scoped>
.session-sidebar {
  display: flex;
  flex-direction: column;
  width: 260px;
  background: #fdfaf5;
  border-right: 1px solid #e8e0d5;
  height: 100%;
}

header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1rem 1.2rem;
  border-bottom: 1px solid #f0ece5;
}

header h3 {
  margin: 0;
  font-size: 1.1rem;
  color: #3b3531;
}

.icon-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  color: #8c827a;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0.3rem;
  border-radius: 6px;
  transition: all 0.2s;
}

.icon-btn:hover {
  background: #f0ece5;
  color: #3b3531;
}

.session-list {
  list-style: none;
  margin: 0;
  padding: 0.5rem;
  overflow-y: auto;
  flex: 1;
}

.session-list li {
  margin-bottom: 0.3rem;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.2s;
}

.session-list li:hover {
  background: #f4efeb;
}

.session-list li.active {
  background: #eae3d9;
}

.mcp-nav-section {
  padding: 1rem 0.5rem 0.5rem 0.5rem;
}

.mcp-nav-btn {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  width: 100%;
  background: transparent;
  border: none;
  cursor: pointer;
  color: #5f5752;
  font-size: 0.95rem;
  padding: 0.6rem 0.8rem;
  border-radius: 8px;
  transition: background 0.2s;
  text-align: left;
}

.mcp-nav-btn:hover {
  background: #f4efeb;
}

.mcp-nav-btn.active {
  background: #eae3d9;
  font-weight: 500;
  color: #2b2623;
}

.nav-divider {
  height: 1px;
  background-color: #f0ece5;
  margin-top: 0.5rem;
  margin-bottom: 0.5rem;
}

.session-list li.active .title {
  font-weight: 500;
  color: #2b2623;
}

.view-mode, .edit-mode {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.6rem 0.8rem;
  height: 2.4rem;
}

.title {
  font-size: 0.9rem;
  color: #5f5752;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}

.actions {
  display: flex;
  gap: 0.2rem;
  opacity: 0;
}

.session-list li:hover .actions {
  opacity: 1;
}

.action-btn:hover {
  background: #eae3d9;
}

.delete-btn:hover {
  color: #d32f2f;
}

.edit-mode input {
  width: 100%;
  border: 1px solid #c8bcae;
  border-radius: 4px;
  padding: 0.2rem 0.4rem;
  font-size: 0.9rem;
  outline: none;
  background: #fff;
}

.empty {
  padding: 1rem;
  text-align: center;
  color: #a49a8f;
  font-size: 0.9rem;
  cursor: default !important;
}
.empty:hover {
  background: transparent !important;
}

.sidebar-footer {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  padding: 0.8rem 1rem;
  border-top: 1px solid #f0ece5;
  background: #fdfaf5;
}

.settings-wrapper {
  position: relative;
}

.footer-btn {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  background: transparent;
  border: none;
  cursor: pointer;
  color: #7a7067;
  font-size: 0.9rem;
  padding: 0.5rem;
  border-radius: 6px;
  transition: all 0.2s;
  position: relative;
}

.footer-btn:hover {
  background: #f0ece5;
  color: #3b3531;
}

.settings-menu {
  position: absolute;
  bottom: calc(100% + 8px);
  left: 0;
  background: #fff;
  border: 1px solid #e8e0d5;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.1);
  padding: 0.4rem;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  min-width: 140px;
  z-index: 10;
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  background: transparent;
  border: none;
  cursor: pointer;
  color: #3b3531;
  font-size: 0.9rem;
  padding: 0.6rem 0.8rem;
  border-radius: 6px;
  transition: all 0.2s;
  text-align: left;
  position: relative;
}

.menu-item:hover {
  background: #f4efeb;
}

.menu-badge {
  position: absolute;
  top: 0;
  right: 0;
  transform: translate(25%, -25%);
  border: 2px solid #fdfaf5;
}

.badge {
  position: absolute;
  top: 0;
  right: 0;
  background-color: #e63946;
  color: white;
  font-size: 0.65rem;
  font-weight: 600;
  padding: 0.1rem 0.35rem;
  border-radius: 12px;
  line-height: 1;
  transform: translate(25%, -25%);
}
</style>
