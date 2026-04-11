<script setup>
import { defineProps, ref, watch, onMounted, onBeforeUnmount, computed } from 'vue'
import { Editor, EditorContent } from '@tiptap/vue-3'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import Mention from '@tiptap/extension-mention'
import { appState } from '../../state/appState.js'
import { invoke } from '@tauri-apps/api/core'
import suggestion from './suggestion.js'

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

const isWebSearchEnabled = ref(false)
const editor = ref(null)
const showModelDropdown = ref(false)
const dropdownRef = ref(null)

const availableProfiles = computed(() => {
  return appState.settings.profiles.filter(p => p.enabled)
})

const activeProfile = computed(() => {
  return availableProfiles.value.find(p => p.id === appState.runtime.activeProfileId) || availableProfiles.value[0]
})

function selectProfile(id) {
  appState.runtime.activeProfileId = id
  showModelDropdown.value = false
}

function handleClickOutside(e) {
  if (dropdownRef.value && !dropdownRef.value.contains(e.target)) {
    showModelDropdown.value = false
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutside)

  editor.value = new Editor({
    extensions: [
      StarterKit.configure({
        // Disable features we don't need for a simple chat input
        heading: false,
        bulletList: false,
        orderedList: false,
        codeBlock: false,
        blockquote: false,
        horizontalRule: false,
      }),
      Placeholder.configure({
        placeholder: 'Message...',
      }),
      Mention.configure({
        HTMLAttributes: {
          class: 'mention',
        },
        suggestion,
      }),
    ],
    content: props.initialText,
    onUpdate: ({ editor }) => {
      // Just emit HTML or text for draft. We'll extract proper text on send.
      emit('update:draft', editor.getHTML())
    },
  })
})

onBeforeUnmount(() => {
  document.removeEventListener('click', handleClickOutside)

  if (editor.value) {
    editor.value.destroy()
  }
})

watch(() => props.initialText, (val) => {
  if (editor.value && val !== editor.value.getHTML()) {
    editor.value.commands.setContent(val)
  }
})

function toggleWebSearch() {
  if (!appState.settings.tools?.braveSearchApiKey) {
    alert('Please configure Brave Search API Key in Settings first')
    return
  }
  isWebSearchEnabled.value = !isWebSearchEnabled.value
}

function handleKeydown(e) {
  // If suggestion popup is open, don't submit on enter
  if (e.key === 'Enter' && !e.shiftKey) {
    // We need to check if the suggestion is active. 
    // We can rely on a class or state from suggestion.js, but simpler:
    const popup = document.querySelector('.tippy-box')
    if (popup) return // let tippy handle it

    e.preventDefault()
    send()
  }
}

function send() {
  if (props.disabled || !editor.value) return
  
  const text = editor.value.getText()
  if (!text.trim()) return

  // Extract explicit skill ID if present
  let explicitSkillId = null
  const json = editor.value.getJSON()
  
  // Very basic traversal to find the first mention node
  function findMention(node) {
    if (node.type === 'mention') {
      return node.attrs.id
    }
    if (node.content) {
      for (const child of node.content) {
        const id = findMention(child)
        if (id) return id
      }
    }
    return null
  }
  
  if (json.content) {
    for (const node of json.content) {
      const id = findMention(node)
      if (id) {
        explicitSkillId = id
        break
      }
    }
  }

  // Use tiptap text serialization to remove the @mention text from the user's prompt
  let cleanText = ''
  if (explicitSkillId && json.content) {
    // If there is an explicit skill mention, filter out the mention node
    const cleanJson = {
      type: 'doc',
      content: json.content.map(block => {
        if (!block.content) return block;
        return {
          ...block,
          content: block.content.filter(inlineNode => inlineNode.type !== 'mention')
        };
      })
    };
    
    // Load it into a temporary editor to get text, or just build text manually.
    // Simple plain text extractor:
    function extractText(node) {
      if (node.type === 'text') return node.text || '';
      if (node.type === 'hardBreak') return '\n';
      if (node.content) return node.content.map(extractText).join('');
      return '';
    }
    
    cleanText = cleanJson.content.map(extractText).join('\n').trim();
  } else {
    cleanText = text;
  }

  if (!cleanText.trim()) return;

  emit('send', { 
    text: cleanText.trim(), 
    webSearch: isWebSearchEnabled.value,
    explicitSkillId: explicitSkillId
  })
  
  editor.value.commands.clearContent()
  emit('update:draft', '')
}
</script>

<template>
  <div class="composer">
    <div class="input-container">
      <div class="editor-wrapper" @keydown="handleKeydown">
        <editor-content :editor="editor" class="composer-input" />
      </div>

      <button 
        class="send-btn" 
        :disabled="disabled || (editor && !editor.getText().trim())" 
        @click="send"
        title="Send message"
      >
        <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
          <line x1="22" y1="2" x2="11" y2="13"></line>
          <polygon points="22 2 15 22 11 13 2 9 22 2"></polygon>
        </svg>
      </button>
    </div>

    <div class="composer-footer">
      <div class="footer-left">
        <div class="model-selector-wrapper" ref="dropdownRef">
          <button class="model-selector-btn" @click.stop="showModelDropdown = !showModelDropdown" type="button" title="Switch Model">
            <svg class="sparkle-icon" viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
              <path d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275Z"></path>
              <path d="M5 3v4"></path>
              <path d="M19 17v4"></path>
              <path d="M3 5h4"></path>
              <path d="M17 19h4"></path>
            </svg>
            <div class="model-info">
              <span class="model-name">{{ activeProfile?.name || '选择模型' }}</span>
              <span class="model-desc" v-if="activeProfile">{{ activeProfile.default_model }}</span>
            </div>
            <svg class="chevron-icon" :class="{ 'is-open': showModelDropdown }" viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="6 9 12 15 18 9"></polyline>
            </svg>
          </button>
          
          <transition name="dropdown-fade">
            <div class="model-dropdown-menu" v-if="showModelDropdown">
              <div 
                class="model-option" 
                v-for="p in availableProfiles" 
                :key="p.id" 
                :class="{ 'is-active': p.id === activeProfile?.id }"
                @click="selectProfile(p.id)"
              >
                <div class="model-option-name">{{ p.name }}</div>
                <div class="model-option-desc">{{ p.default_model }}</div>
                <svg v-if="p.id === activeProfile?.id" class="check-icon" viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="20 6 9 17 4 12"></polyline>
                </svg>
              </div>
            <div v-if="availableProfiles.length === 0" class="model-option-empty">
                无可用模型，请前往设置添加
              </div>
            </div>
          </transition>
        </div>

        <button 
          class="tool-btn" 
          :class="{ active: isWebSearchEnabled }"
          @click="toggleWebSearch"
          title="Toggle Web Search"
        >
          <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="2" y1="12" x2="22" y2="12"></line>
            <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path>
          </svg>
        </button>
      </div>

      <div class="hint">
        <span class="shortcut">↵ Send</span>
        <span class="shortcut">⇧↵ New line</span>
      </div>
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
  width: 100%;
}

.composer-footer {
  width: 100%;
  max-width: 1200px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 0.5rem;
  padding: 0 0.5rem;
  margin-left: auto;
  margin-right: auto;
}

.footer-left {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.model-selector-wrapper {
  position: relative;
}

.model-selector-btn {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  background: transparent;
  border: 1px solid transparent;
  padding: 0.4rem 0.6rem;
  border-radius: 8px;
  color: #554f4b;
  cursor: pointer;
  transition: all 0.2s ease;
  font-family: inherit;
  text-align: left;
}

.model-selector-btn:hover {
  background: #f0ece5;
  color: #2b2623;
}

.model-info {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
}

.sparkle-icon {
  color: #2d6a4f;
}

.model-name {
  font-size: 0.85rem;
  font-weight: 500;
  line-height: 1;
}

.model-desc {
  font-size: 0.7rem;
  color: #a49a8f;
  line-height: 1;
}

.chevron-icon {
  color: #a49a8f;
  transition: transform 0.2s ease;
  margin-left: 0.2rem;
}

.chevron-icon.is-open {
  transform: rotate(180deg);
}

.model-dropdown-menu {
  position: absolute;
  bottom: calc(100% + 0.5rem);
  left: 0;
  background: #ffffff;
  border: 1px solid #e8e0d5;
  border-radius: 12px;
  box-shadow: 0 8px 24px rgba(0,0,0,0.08), 0 2px 8px rgba(0,0,0,0.04);
  min-width: 240px;
  z-index: 100;
  padding: 0.4rem;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  transform-origin: bottom left;
}

.dropdown-fade-enter-active,
.dropdown-fade-leave-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}

.dropdown-fade-enter-from,
.dropdown-fade-leave-to {
  opacity: 0;
  transform: translateY(8px) scale(0.98);
}

.model-option {
  display: flex;
  flex-direction: column;
  padding: 0.6rem 0.8rem;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.15s ease;
  position: relative;
}

.model-option:hover {
  background: #f5f1eb;
}

.model-option.is-active {
  background: #eef5f1;
}

.model-option-name {
  font-size: 0.85rem;
  font-weight: 500;
  color: #2b2623;
  margin-bottom: 0.1rem;
}

.model-option-desc {
  font-size: 0.75rem;
  color: #8c8278;
}

.check-icon {
  position: absolute;
  right: 0.8rem;
  top: 50%;
  transform: translateY(-50%);
  color: #2d6a4f;
}

.model-option-empty {
  padding: 0.8rem;
  font-size: 0.85rem;
  color: #8c8278;
  text-align: center;
}

.input-container {
  width: 100%;
  max-width: 1200px;
  position: relative;
  background: #fff;
  border: 1px solid #d6ccbf;
  border-radius: 12px;
  box-shadow: 0 4px 12px rgba(39, 31, 27, 0.04);
  display: flex;
  padding: 0.5rem;
  transition: border-color 0.2s, box-shadow 0.2s;
  margin: 0 auto;
}

.input-container:focus-within {
  border-color: #2d6a4f;
  box-shadow: 0 4px 12px rgba(45, 106, 79, 0.1);
}

.editor-wrapper {
  flex: 1;
  display: flex;
  flex-direction: column;
  max-height: 200px;
  overflow-y: auto;
}

.composer-input {
  padding: 0.6rem 0.8rem;
  font-family: inherit;
  font-size: 1rem;
  color: #2b2623;
  line-height: 1.4;
  min-height: 24px;
}

/* Tiptap styles */
:deep(.ProseMirror) {
  outline: none;
  word-wrap: break-word;
  white-space: pre-wrap;
  -webkit-font-variant-ligatures: none;
  font-variant-ligatures: none;
}

:deep(.ProseMirror p) {
  margin: 0;
}

:deep(.ProseMirror p.is-editor-empty:first-child::before) {
  color: #a49a8f;
  content: attr(data-placeholder);
  float: left;
  height: 0;
  pointer-events: none;
}

:deep(.mention) {
  background-color: #e6f2ed;
  color: #2d6a4f;
  border-radius: 0.4rem;
  padding: 0.1rem 0.3rem;
  font-weight: 500;
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

.tool-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  color: #a49a8f;
  border: 1px solid transparent;
  padding: 0.4rem;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s ease;
  font-family: inherit;
}

.tool-btn:hover {
  background: #f0ece5;
  color: #2b2623;
}

.tool-btn.active {
  color: #2d6a4f;
  background: #e6f2ed;
}

.hint {
  display: flex;
  gap: 1rem;
  padding: 0;
}

.shortcut {
  font-size: 0.75rem;
  color: #a49a8f;
}
</style>
