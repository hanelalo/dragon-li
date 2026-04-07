<script setup>
import { defineProps, ref, watch, nextTick } from 'vue'

const props = defineProps({
  disabled: {
    type: Boolean,
    default: false
  },
  initialText: {
    type: String,
    default: ''
  }
})

const emit = defineEmits(['send', 'update:draft'])

const input = ref(props.initialText)
const textareaRef = ref(null)

watch(() => props.initialText, (val) => {
  input.value = val
  nextTick(autoResize)
})

function autoResize() {
  const el = textareaRef.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = el.scrollHeight + 'px'
}

function handleInput() {
  emit('update:draft', input.value)
  autoResize()
}

function handleKeydown(e) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    send()
  }
}

function send() {
  if (props.disabled || !input.value.trim()) return
  emit('send', input.value.trim())
  input.value = ''
  emit('update:draft', '')
  nextTick(() => {
    if (textareaRef.value) {
      textareaRef.value.style.height = 'auto'
    }
  })
}
</script>

<template>
  <div class="composer">
    <div class="input-container">
      <textarea
        ref="textareaRef"
        v-model="input"
        @input="handleInput"
        @keydown="handleKeydown"
        placeholder="Message..."
        rows="1"
        class="composer-input"
      ></textarea>
      <button 
        class="send-btn" 
        :disabled="disabled || !input.trim()" 
        @click="send"
        title="Send message"
      >
        <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
          <line x1="22" y1="2" x2="11" y2="13"></line>
          <polygon points="22 2 15 22 11 13 2 9 22 2"></polygon>
        </svg>
      </button>
    </div>
    <div class="hint">
      <span class="shortcut">↵ Send</span>
      <span class="shortcut">⇧↵ New line</span>
    </div>
  </div>
</template>

<style scoped>
.composer {
  padding: 1.5rem;
  background: #fdfaf5;
  border-top: 1px solid #f0ece5;
  display: flex;
  flex-direction: column;
  align-items: center;
}

.input-container {
  width: 100%;
  max-width: 800px;
  position: relative;
  background: #fff;
  border: 1px solid #d6ccbf;
  border-radius: 12px;
  box-shadow: 0 4px 12px rgba(39, 31, 27, 0.04);
  display: flex;
  padding: 0.5rem;
  transition: border-color 0.2s, box-shadow 0.2s;
}

.input-container:focus-within {
  border-color: #2d6a4f;
  box-shadow: 0 4px 12px rgba(45, 106, 79, 0.1);
}

.composer-input {
  flex: 1;
  border: none;
  background: transparent;
  resize: none;
  padding: 0.6rem 0.8rem;
  font-family: inherit;
  font-size: 1rem;
  color: #2b2623;
  outline: none;
  max-height: 200px;
  overflow-y: auto;
  line-height: 1.4;
  box-sizing: border-box;
}

.composer-input::placeholder {
  color: #a49a8f;
}

.composer-input:disabled {
  background: transparent;
  color: #2b2623;
  cursor: text;
}

.send-btn {
  background: #2d6a4f;
  color: #fff;
  border: none;
  width: 36px;
  height: 36px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  margin-left: 0.5rem;
  align-self: flex-end;
  transition: background 0.2s;
}

.send-btn:hover:not(:disabled) {
  background: #1f4a36;
}

.send-btn:disabled {
  background: #e8e0d5;
  color: #a49a8f;
  cursor: not-allowed;
}

.hint {
  width: 100%;
  max-width: 800px;
  display: flex;
  justify-content: flex-end;
  gap: 1rem;
  padding: 0.4rem 0.5rem 0;
}

.shortcut {
  font-size: 0.75rem;
  color: #a49a8f;
}
</style>
