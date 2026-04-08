<script setup>
import { ref, onMounted, onUnmounted, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { confirm as tauriConfirm } from '@tauri-apps/plugin-dialog'
import { appState } from '../state/appState'
import SessionSidebar from '../components/chat/SessionSidebar.vue'
import MessageTimeline from '../components/chat/MessageTimeline.vue'
import Composer from '../components/chat/Composer.vue'

const sessionList = ref([])
const messagesBySession = ref({})
const currentDraft = ref('')
const sendingState = ref(false)
const lastError = ref(null)

let unlistenStream = null

const messages = computed(() => {
  return messagesBySession.value[activeSessionId.value] || []
})

const activeSessionId = computed({
  get: () => appState.runtime.activeSessionId,
  set: (val) => { appState.runtime.activeSessionId = val }
})

const availableProfiles = computed(() => {
  return appState.settings.profiles.filter(p => p.enabled)
})

async function loadConfig() {
  try {
    const res = await invoke('config_get')
    if (res && res.ok) {
      const config = res.data?.config || {}
      const profiles = config.profiles || []
      appState.settings.profiles = profiles.map(p => ({
        id: p.id,
        name: p.name,
        enabled: p.enabled,
        default_model: p.default_model
      }))
      
      appState.settings.tools.braveSearchApiKey = config.tools?.brave_search_api_key || ''
      
      // Update active profile if it was deleted or disabled, or not set
      if (!profiles.find(p => p.id === appState.runtime.activeProfileId && p.enabled)) {
        const firstEnabled = profiles.find(p => p.enabled)
        appState.runtime.activeProfileId = firstEnabled ? firstEnabled.id : ''
      }
    }
  } catch (err) {
    console.error('Failed to load config', err)
  }
}

async function loadSessions() {
  try {
    const res = await invoke('session_list')
    sessionList.value = res?.sessions || res?.data?.sessions || []
    // 移除自动选中第一个会话的逻辑
  } catch (err) {
    console.error('Failed to load sessions', err)
  }
}

async function loadMessages(sessionId) {
  if (!sessionId) return
  if (messagesBySession.value[sessionId]) return // use cache
  try {
    const res = await invoke('message_list', { sessionId })
    messagesBySession.value[sessionId] = res?.messages || res?.data?.messages || []
  } catch (err) {
    console.error('Failed to load messages', err)
  }
}

watch(activeSessionId, async (newId) => {
  if (newId) {
    await loadMessages(newId)
  }
})

onMounted(async () => {
  // Try to create seed data just in case db is empty (for demo purposes)
  await invoke('db_init').catch(() => {})
  
  await loadConfig()
  await loadSessions()
  if (activeSessionId.value) {
    await loadMessages(activeSessionId.value)
  }

  unlistenStream = await listen('chat_stream_event', async (event) => {
    const payload = event.payload
    if (!payload || !payload.event) return

    const { request_id, event: streamEvent } = payload
    const msgIndex = messages.value.findIndex(m => m.request_id === request_id)
    if (msgIndex === -1) return

    // Create a new reference so Vue's reactivity detects changes, or we can just mutate the object.
    const msg = messages.value[msgIndex]

    switch (streamEvent.type) {
      case 'reasoning':
        msg.reasoning_md = (msg.reasoning_md || '') + streamEvent.text
        messages.value[msgIndex] = msg
        break
      case 'delta':
        msg.content_md = (msg.content_md || '') + streamEvent.text
        messages.value[msgIndex] = msg
        break
      case 'done':
        msg.status = 'ok'
        msg.tokens_in = streamEvent.payload?.tokens_in || 0
        msg.tokens_out = streamEvent.payload?.tokens_out || 0
        msg.latency_ms = streamEvent.payload?.latency_ms || 0
        sendingState.value = false
        break
      case 'aborted':
        msg.status = 'failed'
        msg.error_code = streamEvent.payload.code
        msg.error_message = streamEvent.payload.message
        sendingState.value = false
        break
    }
  })
})

onUnmounted(() => {
  if (unlistenStream) unlistenStream()
})

async function handleCreateSession() {
  const id = `s_${Date.now()}`
  const now = new Date().toISOString()
  const newSession = {
    id,
    title: 'New Chat',
    status: 'active',
    default_provider: null,
    default_model: null,
    created_at: now,
    updated_at: now
  }
  try {
    await invoke('session_create', { session: newSession })
    await loadSessions()
    activeSessionId.value = id
  } catch (err) {
    console.error('Create session failed', err)
  }
}

async function handleRenameSession({ id, title }) {
  try {
    await invoke('session_update_title', { sessionId: id, title })
    await loadSessions()
  } catch (err) {
    console.error('Rename session failed', err)
  }
}

async function handleDeleteSession(id) {
  try {
    const confirmed = await tauriConfirm('Are you sure you want to archive/delete this session?', { title: 'Dragon Li', kind: 'warning' });
    if (!confirmed) return;
    const res = await invoke('session_soft_delete', { sessionId: id, deletedAt: new Date().toISOString() });
    if (res && !res.ok) {
      console.error("Delete failed:", res.error);
      return;
    }
    if (activeSessionId.value === id) {
      activeSessionId.value = ''
    }
    delete messagesBySession.value[id]
    await loadSessions()
  } catch (err) {
    console.error('Delete session failed', err)
  }
}

async function handleSendMessage(payload, retryMessage = null) {
  let text = typeof payload === 'string' ? payload : payload.text
  let webSearch = typeof payload === 'string' ? false : (payload.webSearch || false)

  if (!activeSessionId.value) {
    // If no active session, create a new one first
    const id = `s_${Date.now()}`
    const now = new Date().toISOString()
    const newSession = {
      id,
      title: 'New Chat',
      status: 'active',
      default_provider: null,
      default_model: null,
      created_at: now,
      updated_at: now
    }
    try {
      await invoke('session_create', { session: newSession })
      await loadSessions()
      activeSessionId.value = id
    } catch (err) {
      console.error('Create session failed', err)
      return
    }
  }
  
  if (!messagesBySession.value[activeSessionId.value]) {
    messagesBySession.value[activeSessionId.value] = []
  }
  
  // Calculate history before modifying messages array
  let historyMessages = []
  if (retryMessage) {
    const retryIdx = messages.value.findIndex(m => m.id === retryMessage.id)
    if (retryIdx > 0) {
      historyMessages = messages.value.slice(0, retryIdx - 1)
    }
  } else {
    historyMessages = [...messages.value]
  }

  const history = historyMessages
    .filter(m => m.status === 'ok' && m.content_md && m.content_md.trim().length > 0)
    .map(m => ({
      role: m.role,
      content: m.content_md
    }))

  // Base timestamp for current message pair
  const baseTimestampMs = Date.now()

  // Create User Message
  if (!retryMessage) {
    const userMsg = {
      id: `m_user_${baseTimestampMs}`,
      session_id: activeSessionId.value,
      role: 'user',
      content_md: text,
      reasoning_md: null,
      provider: null,
      model: null,
      tokens_in: null,
      tokens_out: null,
      latency_ms: null,
      parent_message_id: null,
      status: 'ok',
      error_code: null,
      error_message: null,
      retryable: null,
      created_at: new Date(baseTimestampMs).toISOString()
    }
    messages.value.push(userMsg)
    await invoke('message_create', { message: userMsg }).catch(console.error)

    // Trigger title generation if this is the first message in the session
    // Refresh the session list so the newly created session has the updated title in the UI
    const currentSession = sessionList.value.find(s => s.id === activeSessionId.value)
    // Calculate the number of user messages in this session
    // `historyMessages` contains the history before this new message is added.
    const userMsgCount = historyMessages.filter(m => m.role === 'user').length + 1;
    
    // Allow matching initial titles like 'New Chat' or '新对话'
    // Also explicitly check userMsgCount === 1, just in case title was never changed manually.
    if (currentSession && userMsgCount === 1) {
      // Don't wait for summarize to finish
      invoke('chat_summarize_title', { 
        profileId: appState.runtime.activeProfileId || 'openai-main',
        userText: text 
      }).then(async (res) => {
        if (res && res.ok && res.data.title) {
          const title = res.data.title;
          await invoke('session_update_title', { sessionId: activeSessionId.value, title })
          currentSession.title = title
          await loadSessions()
        }
      }).catch(err => {
        console.error("Title generation failed:", err)
      })
    }
  }

  // Create Assistant Placeholder
  const reqId = retryMessage ? retryMessage.id.replace('m_ast_', '') : `req_${Date.now()}`
  const astMsg = {
    id: `m_ast_${reqId}`,
    session_id: activeSessionId.value,
    role: 'assistant',
    content_md: '',
    reasoning_md: '',
    provider: null,
    model: null,
    tokens_in: null,
    tokens_out: null,
    latency_ms: null,
    parent_message_id: null,
    status: 'streaming',
    error_code: null,
    error_message: null,
    retryable: null,
    created_at: new Date(baseTimestampMs + 10).toISOString(),
    request_id: reqId // For UI tracking
  }
  
  if (retryMessage) {
    // replace failed message with new placeholder
    const idx = messages.value.findIndex(m => m.id === retryMessage.id)
    if (idx !== -1) {
      messages.value[idx] = astMsg
    }
  } else {
    messages.value.push(astMsg)
  }

  // Pre-insert the assistant message to db with status 'streaming'
  await invoke('message_create', { message: astMsg }).catch(console.error)

  sendingState.value = true
  lastError.value = null

  // Use a hardcoded profile id if activeProfileId is empty
  const profileId = appState.runtime.activeProfileId || 'openai-main'
  
  try {
    const res = await invoke('chat_send', {
      request: {
        profile_id: profileId,
        request_id: reqId,
        session_id: activeSessionId.value,
        model: null,
        enable_web_search: webSearch,
        prompt: {
          system: '',
          runtime: '',
          memory: '',
          user: text || retryMessage?.content_md || ''
        },
        history
      }
    })
    
    // If chat_send completes normally, status should be updated via streaming 'done'
    // But in case of an error returned immediately without aborted event:
    if (!res.ok) {
      astMsg.status = 'failed'
      astMsg.error_code = res.error?.code || 'UNKNOWN'
      astMsg.error_message = res.error?.message || String(res.error)
      sendingState.value = false
      await invoke('message_create', { message: { ...astMsg, created_at: new Date(baseTimestampMs + 10).toISOString() } }).catch(console.error)
    }
  } catch (err) {
    astMsg.status = 'failed'
    astMsg.error_code = 'UNKNOWN_ERROR'
    astMsg.error_message = String(err)
    sendingState.value = false
    await invoke('message_create', { message: { ...astMsg, created_at: new Date(baseTimestampMs + 10).toISOString() } }).catch(console.error)
  }
}

function handleRetry(failedMessage) {
  // Find the user message before this failed message to get the text
  const idx = messages.value.findIndex(m => m.id === failedMessage.id)
  let userText = ''
  if (idx > 0 && messages.value[idx - 1].role === 'user') {
    userText = messages.value[idx - 1].content_md
  }
  handleSendMessage(userText, failedMessage)
}

function updateDraft(val) {
  currentDraft.value = val
}
</script>

<template>
  <div class="chat-page-container">
    <SessionSidebar
      :sessions="sessionList"
      :active-id="activeSessionId"
      @select="id => activeSessionId = id"
      @create="handleCreateSession"
      @rename="handleRenameSession"
      @delete="handleDeleteSession"
    />
    <main class="chat-main">
      <header class="main-header">
        <div class="header-left">
          <h2>{{ sessionList.find(s => s.id === activeSessionId)?.title || (activeSessionId ? 'Chat' : 'Start a New Chat') }}</h2>
        </div>
        <div class="header-right">
          <label>模型:</label>
          <select class="profile-select" v-model="appState.runtime.activeProfileId">
            <option v-for="p in availableProfiles" :key="p.id" :value="p.id">
              {{ p.name }} ({{ p.default_model }})
            </option>
            <option v-if="availableProfiles.length === 0" value="" disabled>无可用 Profile，请前往设置添加</option>
          </select>
        </div>
      </header>
      <MessageTimeline 
        :messages="messages"
        @retry="handleRetry" 
      />
      <Composer
        :disabled="sendingState"
        :initial-text="currentDraft"
        @send="handleSendMessage"
        @update:draft="updateDraft"
      />
    </main>
  </div>
</template>

<style scoped>
.chat-page-container {
  display: flex;
  height: 100%;
  width: 100%;
  overflow: hidden;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.05);
}

.chat-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: #fdfaf5;
  min-width: 0;
  min-height: 0;
}

.main-header {
  padding: 1rem 1.5rem;
  background: #fdfaf5;
  border-bottom: 1px solid #f0ece5;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.main-header h2 {
  margin: 0;
  font-size: 1.2rem;
  color: #3b3531;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.85rem;
  color: #666;
}

.profile-select {
  padding: 0.2rem 0.5rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #fdfdfd;
  font-size: 0.85rem;
  color: #333;
}
.profile-select:focus {
  outline: none;
  border-color: #888;
}
</style>
