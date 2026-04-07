<script setup>
import { defineProps, ref, watch, nextTick } from 'vue'
import { marked } from 'marked'
import DOMPurify from 'dompurify'

const props = defineProps({
  messages: {
    type: Array,
    required: true
  }
})

const timelineRef = ref(null)

// Auto-scroll when messages update
watch(
  () => props.messages,
  async () => {
    await nextTick()
    if (timelineRef.value) {
      timelineRef.value.scrollTop = timelineRef.value.scrollHeight
    }
  },
  { deep: true }
)

function formatTime(isoString) {
  if (!isoString) return ''
  const d = new Date(isoString)
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

// Convert markdown to sanitized HTML
function renderMarkdown(mdText) {
  if (!mdText) return ''
  // marked.parse returns a string or promise. By default it's synchronous.
  const rawHtml = marked.parse(mdText)
  return DOMPurify.sanitize(rawHtml, { ADD_ATTR: ['target', 'class', 'open'] })
}

const emit = defineEmits(['retry'])

const copiedMessageId = ref(null)

async function copyText(text, msgId) {
  if (!text) return
  try {
    await navigator.clipboard.writeText(text)
    copiedMessageId.value = msgId
    setTimeout(() => {
      if (copiedMessageId.value === msgId) {
        copiedMessageId.value = null
      }
    }, 2000)
  } catch (err) {
    console.error('Failed to copy text', err)
  }
}
</script>

<template>
  <div class="message-timeline" ref="timelineRef">
    <div v-if="messages.length === 0" class="empty-state">
      <div class="icon">✨</div>
      <p>How can I help you today?</p>
    </div>
    
    <div v-for="msg in messages" :key="msg.id" :class="['message', msg.role]">
      <div class="avatar" v-if="msg.role === 'user'">
        U
      </div>
      <div class="avatar ai-avatar" v-else>
        <img src="../../assets/logo.png" alt="AI" />
      </div>
      <div class="content-box">
        <div class="header">
          <span class="role">{{ msg.role === 'user' ? 'You' : 'Assistant' }}</span>
          <span class="time">{{ formatTime(msg.created_at) }}</span>
        </div>
        <div class="content">
          <div v-if="msg.reasoning_md" class="markdown-body reasoning-block" :class="{'streaming-reasoning': msg.status === 'streaming'}">
            <details :open="msg.status === 'streaming' || true">
              <summary>思考过程</summary>
              <div v-html="renderMarkdown(msg.reasoning_md)"></div>
            </details>
          </div>
          <div v-if="msg.content_md" class="markdown-body" v-html="renderMarkdown(msg.content_md)"></div>
          <span v-else-if="msg.status === 'streaming'" class="streaming-indicator">
            <span class="dot"></span><span class="dot"></span><span class="dot"></span>
          </span>
        </div>
        
        <!-- Error State & Retry -->
        <div v-if="msg.status === 'failed'" class="error-block">
          <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="12" y1="8" x2="12" y2="12"></line>
            <line x1="12" y1="16" x2="12.01" y2="16"></line>
          </svg>
          <span class="error-text">
            Error: {{ msg.error_code || 'Unknown' }}
            <span v-if="msg.request_id" class="req-id">(Req: {{ msg.request_id }})</span>
          </span>
          <button class="retry-btn" @click="emit('retry', msg)">Retry</button>
        </div>
          <div v-if="msg.status === 'ok' && msg.role === 'assistant'" class="message-footer">
            <div v-if="msg.latency_ms" class="usage-stats">
              ⏱ {{ (msg.latency_ms / 1000).toFixed(1) }}s · 🪙 {{ msg.tokens_in }} in / {{ msg.tokens_out }} out
            </div>
            <div class="message-actions">
              <button class="icon-btn copy-btn" @click="copyText(msg.content_md, msg.id)" title="Copy message">
                <svg v-if="copiedMessageId === msg.id" viewBox="0 0 24 24" width="14" height="14" stroke="#2d6a4f" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="20 6 9 17 4 12"></polyline>
                </svg>
                <svg v-else viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                </svg>
              </button>
            </div>
          </div>
          <div v-else-if="msg.status === 'ok' && msg.role === 'user'" class="message-footer user-footer">
            <div class="message-actions">
              <button class="icon-btn copy-btn" @click="copyText(msg.content_md, msg.id)" title="Copy message">
                <svg v-if="copiedMessageId === msg.id" viewBox="0 0 24 24" width="14" height="14" stroke="#2d6a4f" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="20 6 9 17 4 12"></polyline>
                </svg>
                <svg v-else viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                </svg>
              </button>
            </div>
          </div>
        </div>
      </div>
  </div>
</template>

<style scoped>
.message-timeline {
  flex: 1;
  overflow-y: auto;
  padding: 1.5rem;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  background: #fdfaf5;
  min-height: 0;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: #8c827a;
}

.empty-state .icon {
  font-size: 2.5rem;
  margin-bottom: 1rem;
}

.empty-state p {
  font-size: 1.2rem;
  font-weight: 500;
}

.message {
  display: flex;
  gap: 1rem;
  max-width: 1200px;
  margin: 0 auto;
  width: 100%;
}

.message.user {
  flex-direction: row-reverse;
}

.avatar {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: bold;
  font-size: 0.9rem;
  flex-shrink: 0;
  overflow: hidden;
}

.ai-avatar img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.message.user .avatar {
  background: #d4e4da;
  color: #2b5c3e;
}

.message.assistant .avatar {
  background: transparent;
  color: #4a413a;
}

.content-box {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  max-width: 80%;
  min-width: 0; /* Prevents flex children from blowing out the container */
}

.message.user .content-box {
  align-items: flex-end;
}

.header {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
  margin-bottom: 0.2rem;
}

.message.user .header {
  flex-direction: row-reverse;
}

.role {
  font-weight: 600;
  font-size: 0.85rem;
  color: #3b3531;
}

.time {
  font-size: 0.75rem;
  color: #a49a8f;
}

.content {
  background: #fff;
  padding: 0.8rem 1rem;
  border-radius: 12px;
  border: 1px solid #e8e0d5;
  box-shadow: 0 2px 4px rgba(0,0,0,0.02);
  line-height: 1.5;
  color: #2b2623;
  font-size: 0.95rem;
  word-wrap: break-word;
  overflow-x: auto; /* Enable scrolling for wide content like tables */
  max-width: 100%;
  user-select: text;
  -webkit-user-select: text;
}

.message-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 0.4rem;
  min-height: 24px;
}

.user-footer {
  justify-content: flex-end;
}

.usage-stats {
  font-size: 0.7rem;
  color: #9fa0a1;
  display: flex;
  gap: 0.5rem;
  align-items: center;
}

/* Base markdown styles to make it look native inside the chat bubble */
:deep(.markdown-body) {
  font-family: inherit;
  font-size: inherit;
  color: inherit;
  line-height: 1.6;
  white-space: normal; /* Override pre-wrap for standard HTML elements */
}

:deep(.markdown-body p) {
  margin-top: 0;
  margin-bottom: 0.75rem;
}
:deep(.markdown-body p:last-child) {
  margin-bottom: 0;
}

:deep(.markdown-body h1),
:deep(.markdown-body h2),
:deep(.markdown-body h3),
:deep(.markdown-body h4) {
  margin-top: 1rem;
  margin-bottom: 0.5rem;
  font-weight: 600;
  line-height: 1.25;
}

:deep(.markdown-body ul),
:deep(.markdown-body ol) {
  margin-top: 0;
  margin-bottom: 0.75rem;
  padding-left: 1.5em;
}

:deep(.markdown-body li) {
  margin-bottom: 0.25rem;
}

:deep(.markdown-body code) {
  background-color: rgba(0, 0, 0, 0.05);
  border-radius: 4px;
  padding: 0.2em 0.4em;
  font-family: ui-monospace, SFMono-Regular, Consolas, 'Liberation Mono', Menlo, monospace;
  font-size: 0.85em;
}

:deep(.markdown-body pre) {
  background-color: #f6f8fa;
  border-radius: 6px;
  padding: 1rem;
  overflow: auto;
  margin-bottom: 0.75rem;
  white-space: pre; /* Code blocks should preserve exact spacing */
  max-width: 100%;
}

:deep(.markdown-body pre code) {
  background-color: transparent;
  padding: 0;
  border-radius: 0;
  font-size: 0.9em;
  color: #24292e;
}

:deep(.markdown-body blockquote) {
  margin: 0 0 0.75rem 0;
  padding: 0 1em;
  color: #6a737d;
  border-left: 0.25em solid #dfe2e5;
}

:deep(.markdown-body a) {
  color: #0366d6;
  text-decoration: none;
}
:deep(.markdown-body a:hover) {
  text-decoration: underline;
}

/* Markdown tables styling */
:deep(.markdown-body table) {
  display: block;
  width: 100%;
  width: max-content;
  max-width: 100%;
  overflow: auto;
  border-spacing: 0;
  border-collapse: collapse;
  margin-top: 0;
  margin-bottom: 1rem;
}

:deep(.markdown-body table th),
:deep(.markdown-body table td) {
  padding: 6px 13px;
  border: 1px solid #dfe2e5;
}

:deep(.markdown-body table tr) {
  background-color: transparent;
  border-top: 1px solid #c6cbd1;
}

:deep(.markdown-body table tr:nth-child(2n)) {
  background-color: rgba(0, 0, 0, 0.02);
}

/* Ensure user message text still looks right if it contains simple markdown */
.message.user .content {
  background: #2d6a4f;
  color: #fff;
  border: none;
  border-top-right-radius: 4px;
}

/* User message specific overrides for markdown */
:deep(.message.user .markdown-body code) {
  background-color: rgba(255, 255, 255, 0.2);
  color: inherit;
}
:deep(.message.user .markdown-body pre) {
  background-color: rgba(0, 0, 0, 0.2);
  color: inherit;
}
:deep(.message.user .markdown-body pre code) {
  color: inherit;
}
:deep(.message.user .markdown-body a) {
  color: #a8d5c2;
}

:deep(.message.user .markdown-body table th),
:deep(.message.user .markdown-body table td) {
  border-color: rgba(255, 255, 255, 0.3);
}

:deep(.message.user .markdown-body table tr) {
  border-color: rgba(255, 255, 255, 0.3);
}

:deep(.message.user .markdown-body table tr:nth-child(2n)) {
  background-color: rgba(255, 255, 255, 0.1);
}

.message.assistant .content {
  border-top-left-radius: 4px;
}

pre {
  margin: 0;
  font-family: inherit;
  white-space: pre-wrap;
}

.streaming-indicator {
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
  height: 1.5rem;
}

.dot {
  width: 6px;
  height: 6px;
  background: #8c827a;
  border-radius: 50%;
  animation: blink 1.4s infinite both;
}

.dot:nth-child(2) { animation-delay: 0.2s; }
.dot:nth-child(3) { animation-delay: 0.4s; }

@keyframes blink {
  0%, 80%, 100% { opacity: 0.3; }
  40% { opacity: 1; }
}

.error-block {
  margin-top: 0.5rem;
  padding: 0.6rem 0.8rem;
  background: #fef2f2;
  border: 1px solid #fecaca;
  border-radius: 8px;
  color: #b91c1c;
  font-size: 0.85rem;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.error-text {
  flex: 1;
}

.req-id {
  color: #991b1b;
  font-size: 0.75rem;
  opacity: 0.8;
  margin-left: 0.3rem;
}

.retry-btn {
  background: #ef4444;
  color: #fff;
  border: none;
  padding: 0.3rem 0.6rem;
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.8rem;
  transition: background 0.2s;
}

.retry-btn:hover {
  background: #dc2626;
}



.message-actions {
  opacity: 0;
  transition: opacity 0.2s;
  display: flex;
  gap: 4px;
}

.content-box:hover .message-actions {
  opacity: 1;
}

.icon-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  color: #a49a8f;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0.3rem;
  border-radius: 6px;
  transition: all 0.2s;
}

.icon-btn:hover {
  background: rgba(0, 0, 0, 0.05);
  color: #3b3531;
}
</style>
