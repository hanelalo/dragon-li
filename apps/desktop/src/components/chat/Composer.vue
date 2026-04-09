<script setup>
import { defineProps, ref, watch, onMounted, onBeforeUnmount } from 'vue'
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

onMounted(() => {
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
  if (editor.value) {
    editor.value.destroy()
  }
})

watch(() => props.initialText, (val) => {
  if (editor.value && val !== editor.value.getHTML()) {
    editor.value.commands.setContent(val)
  }
})

watch(() => props.disabled, (val) => {
  if (editor.value) {
    editor.value.setEditable(!val)
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
      <button 
        class="tool-btn" 
        :class="{ active: isWebSearchEnabled }"
        @click="toggleWebSearch"
        title="Toggle Web Search"
      >
        <svg viewBox="0 0 24 24" width="18" height="18" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"></circle>
          <line x1="2" y1="12" x2="22" y2="12"></line>
          <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path>
        </svg>
      </button>
      
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
  background: transparent;
  color: #a49a8f;
  border: none;
  width: 36px;
  height: 36px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  margin-right: 0.5rem;
  align-self: flex-end;
  transition: color 0.2s, background 0.2s;
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
