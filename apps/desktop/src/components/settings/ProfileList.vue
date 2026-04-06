<script setup>
import { computed } from 'vue'

const props = defineProps({
  profiles: {
    type: Array,
    required: true
  },
  activeProfileId: {
    type: String,
    required: true
  }
})

const emit = defineEmits(['select', 'add'])
</script>

<template>
  <div class="profile-list">
    <div class="header">
      <h2>Profiles</h2>
      <button class="add-btn" @click="emit('add')">新建</button>
    </div>
    
    <ul v-if="profiles.length > 0" class="list">
      <li 
        v-for="profile in profiles" 
        :key="profile.id"
        :class="{ active: profile.id === activeProfileId }"
        @click="emit('select', profile.id)"
      >
        <div class="info">
          <span class="name">{{ profile.name }}</span>
          <span class="provider">{{ profile.provider }}</span>
        </div>
        <div class="status">
          <span v-if="profile.enabled" class="badge enabled">启用</span>
          <span v-else class="badge disabled">禁用</span>
        </div>
      </li>
    </ul>
    <p v-else class="empty">暂无 Profile</p>
  </div>
</template>

<style scoped>
.profile-list {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  border-right: 1px solid #e5dbce;
  padding-right: 1rem;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

h2 {
  margin: 0;
  font-size: 1.1rem;
}

.add-btn {
  background: #2d6a4f;
  color: white;
  border: none;
  border-radius: 6px;
  padding: 0.3rem 0.6rem;
  cursor: pointer;
  font-size: 0.9rem;
}

.list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.list li {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.6rem;
  border: 1px solid #d8cdbd;
  border-radius: 8px;
  cursor: pointer;
  background: #fdfaf5;
  transition: all 0.2s;
}

.list li:hover {
  background: #f5eee4;
}

.list li.active {
  border-color: #2d6a4f;
  background: #e8f0ec;
}

.info {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.name {
  font-weight: 500;
  color: #2f2b28;
}

.provider {
  font-size: 0.8rem;
  color: #746a62;
  text-transform: capitalize;
}

.badge {
  font-size: 0.75rem;
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
}

.badge.enabled {
  background: #d8f3dc;
  color: #1b4332;
}

.badge.disabled {
  background: #e9ecef;
  color: #495057;
}

.empty {
  color: #7a7067;
  text-align: center;
  font-size: 0.9rem;
  margin-top: 2rem;
}
</style>
