<script setup>
import { ref, watch, defineProps, defineExpose } from 'vue'

const props = defineProps({
  items: {
    type: Array,
    required: true,
  },
  command: {
    type: Function,
    required: true,
  },
})

const selectedIndex = ref(0)

watch(() => props.items, () => {
  selectedIndex.value = 0
})

function onKeyDown({ event }) {
  if (event.key === 'ArrowUp') {
    upHandler()
    return true
  }

  if (event.key === 'ArrowDown') {
    downHandler()
    return true
  }

  if (event.key === 'Enter') {
    enterHandler()
    return true
  }

  return false
}

function upHandler() {
  selectedIndex.value = ((selectedIndex.value + props.items.length) - 1) % props.items.length
}

function downHandler() {
  selectedIndex.value = (selectedIndex.value + 1) % props.items.length
}

function enterHandler() {
  selectItem(selectedIndex.value)
}

function selectItem(index) {
  const item = props.items[index]

  if (item) {
    props.command({ id: item.id, label: item.label })
  }
}

defineExpose({ onKeyDown })
</script>

<template>
  <div class="items">
    <template v-if="items.length">
      <button
        class="item"
        :class="{ 'is-selected': index === selectedIndex }"
        v-for="(item, index) in items"
        :key="index"
        @click="selectItem(index)"
      >
        <span class="label">{{ item.label }}</span>
        <span class="desc">{{ item.description }}</span>
      </button>
    </template>
    <div class="item" v-else>
      No skills found
    </div>
  </div>
</template>

<style scoped>
.items {
  background: #fff;
  border: 1px solid #e0dcd5;
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(39, 31, 27, 0.1);
  color: #2b2623;
  font-size: 0.9rem;
  overflow: hidden;
  padding: 0.4rem;
  position: relative;
  display: flex;
  flex-direction: column;
  min-width: 200px;
  max-width: 300px;
}

.item {
  background: transparent;
  border: none;
  border-radius: 6px;
  color: inherit;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  margin: 0;
  padding: 0.4rem 0.6rem;
  text-align: left;
  width: 100%;
  cursor: pointer;
  transition: background 0.1s;
}

.item.is-selected,
.item:hover {
  background-color: #f0ece5;
}

.label {
  font-weight: 600;
  color: #2d6a4f;
  margin-bottom: 0.2rem;
}

.desc {
  font-size: 0.75rem;
  color: #8c8273;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  width: 100%;
}
</style>
