<script setup>
import { defineProps, ref, nextTick } from 'vue'

const props = defineProps({
  sessions: {
    type: Array,
    required: true
  },
  activeId: {
    type: String,
    default: ''
  }
})

const emit = defineEmits(['select', 'create', 'rename', 'delete'])

const editingId = ref(null)
const editTitle = ref('')

function startRename(session) {
  editingId.value = session.id
  editTitle.value = session.title
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
    e.preventDefault()
    submitRename(session)
  } else if (e.key === 'Escape') {
    editingId.value = null
  }
}
</script>

<template>
  <aside class="session-sidebar">
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
            autoFocus
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
</style>
