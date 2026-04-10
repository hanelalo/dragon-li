<script setup>
import { ref, watch, onUnmounted } from 'vue'
import { marked } from 'marked'
import DOMPurify from 'dompurify'

const props = defineProps({
  content: {
    type: String,
    default: ''
  }
})

const renderedHtml = ref('')
let timeoutId = null
let lastRenderTime = 0

function render() {
  if (!props.content) {
    renderedHtml.value = ''
    return
  }
  const rawHtml = marked.parse(props.content)
  renderedHtml.value = DOMPurify.sanitize(rawHtml, { ADD_ATTR: ['target', 'class', 'open'] })
  lastRenderTime = Date.now()
}

watch(() => props.content, () => {
  const now = Date.now()
  if (now - lastRenderTime > 100) {
    if (timeoutId) {
      clearTimeout(timeoutId)
      timeoutId = null
    }
    render()
  } else {
    if (!timeoutId) {
      timeoutId = setTimeout(() => {
        render()
        timeoutId = null
      }, 100 - (now - lastRenderTime))
    }
  }
}, { immediate: true })

onUnmounted(() => {
  if (timeoutId) {
    clearTimeout(timeoutId)
  }
})
</script>

<template>
  <div v-html="renderedHtml"></div>
</template>
