<script setup>
import { ref, onMounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-shell'

const skills = ref([])
const activeSkillId = ref('')
const errorMsg = ref('')

const activeSkill = computed(() => {
  return skills.value.find(s => s.id === activeSkillId.value)
})

const hasSelection = computed(() => !!activeSkill.value)

async function loadSkills() {
  try {
    errorMsg.value = ''
    const res = await invoke('skill_list')
    if (res.ok) {
      skills.value = res.data.skills || []
      if (skills.value.length > 0 && !activeSkillId.value) {
        activeSkillId.value = skills.value[0].id
      }
    } else {
      errorMsg.value = res.error?.message || 'Failed to load skills'
    }
  } catch (err) {
    errorMsg.value = String(err)
  }
}

function handleSelectSkill(id) {
  activeSkillId.value = id
}

async function toggleSkill(skill) {
  const newEnabled = !skill.enabled
  try {
    errorMsg.value = ''
    const res = await invoke('skill_toggle', {
      id: skill.id,
      enabled: newEnabled
    })
    if (res.ok) {
      skill.enabled = newEnabled
    } else {
      errorMsg.value = res.error?.message || 'Failed to toggle skill'
    }
  } catch (err) {
    errorMsg.value = String(err)
  }
}

async function handleRescan() {
  try {
    errorMsg.value = ''
    const res = await invoke('skill_rescan')
    if (res.ok) {
      alert('Rescan triggered successfully. Please refresh the page in a moment.')
      setTimeout(loadSkills, 1000)
    } else {
      errorMsg.value = res.error?.message || 'Failed to trigger rescan'
    }
  } catch (err) {
    errorMsg.value = String(err)
  }
}

async function openSkillFolder(skill) {
  try {
    errorMsg.value = ''
    const infoRes = await invoke('runtime_info')
    if (infoRes.ok && infoRes.data?.runtime_root) {
      const rootPath = infoRes.data.runtime_root
      const skillPath = `${rootPath}/skills/${skill.name}`
      await open(skillPath)
    } else {
      errorMsg.value = 'Failed to get runtime root directory'
    }
  } catch (err) {
    errorMsg.value = `Failed to open folder: ${err}`
  }
}

function goBack() {
  window.location.hash = '/chat'
}

onMounted(() => {
  loadSkills()
})
</script>

<template>
  <section class="skill-page">
    <header class="page-header">
      <div class="header-left">
        <button class="back-btn" @click="goBack" title="返回">
          <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <line x1="19" y1="12" x2="5" y2="12"></line>
            <polyline points="12 19 5 12 12 5"></polyline>
          </svg>
        </button>
        <div>
          <h1>Skills</h1>
          <p>管理系统技能 (Skills) 与专家工作流</p>
        </div>
      </div>
      <div class="header-actions">
        <button class="btn primary-btn" @click="handleRescan">
          <svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="23 4 23 10 17 10"></polyline>
            <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"></path>
          </svg>
          重新扫描
        </button>
      </div>
    </header>

    <div v-if="errorMsg" class="error-banner">
      {{ errorMsg }}
    </div>

    <div class="layout">
      <div class="sidebar">
        <div class="skill-list">
          <div 
            v-for="skill in skills" 
            :key="skill.id" 
            class="list-item"
            :class="{ active: skill.id === activeSkillId }"
            @click="handleSelectSkill(skill.id)"
          >
            <div class="item-content">
              <span class="item-name">{{ skill.name }}</span>
              <span class="status-dot" :class="skill.enabled ? 'enabled' : 'disabled'"></span>
            </div>
          </div>
          <div v-if="skills.length === 0 && !errorMsg" class="empty-list">
            未发现任何技能
          </div>
        </div>
      </div>
      
      <div class="editor-area">
        <div v-if="hasSelection" class="skill-details">
          <div class="details-header">
            <h2>{{ activeSkill.name }}</h2>
            <label class="toggle-switch" title="启用/禁用">
              <input type="checkbox" :checked="activeSkill.enabled" @change="toggleSkill(activeSkill)" />
              <span class="slider"></span>
            </label>
          </div>
          
          <div class="details-content">
            <div class="field-group">
              <label>描述</label>
              <p class="desc-text">{{ activeSkill.description || '无描述' }}</p>
            </div>
            
            <div class="field-group">
              <label>危险等级</label>
              <p class="info-text">{{ activeSkill.risk_level }}</p>
            </div>

            <div class="field-group">
              <label>更新时间</label>
              <p class="info-text">{{ new Date(activeSkill.updated_at).toLocaleString() }}</p>
            </div>

            <div class="actions-footer">
              <button class="btn outline-btn" @click="openSkillFolder(activeSkill)">
                <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                </svg>
                打开文件夹
              </button>
            </div>
          </div>
        </div>
        <div v-else class="no-selection">
          <p>请选择一个技能查看详情，或点击“重新扫描”发现新技能</p>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.skill-page {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  height: 100%;
  padding: 1.5rem;
  box-sizing: border-box;
}

.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.back-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  color: #6f6460;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0.5rem;
  border-radius: 8px;
  transition: background 0.2s, color 0.2s;
}

.back-btn:hover {
  background: #eae3d9;
  color: #2b2623;
}

header h1 { margin: 0; font-size: 1.8rem; }
header p { margin: 0.35rem 0 0; color: #5f5953; }

.error-banner {
  background: #f8d7da;
  color: #721c24;
  padding: 1rem;
  border-radius: 8px;
  border: 1px solid #f5c6cb;
}

.layout {
  display: grid;
  grid-template-columns: 280px 1fr;
  gap: 2rem;
  flex: 1;
  min-height: 0;
}

.sidebar {
  overflow-y: auto;
  border-right: 1px solid #e8e0d5;
  padding-right: 1rem;
}

.editor-area {
  overflow-y: auto;
  padding-right: 1rem;
}

.skill-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.list-item {
  padding: 0.8rem 1rem;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
  border: 1px solid transparent;
}

.list-item:hover {
  background: #f4efeb;
}

.list-item.active {
  background: #fff;
  border-color: #e8e0d5;
  box-shadow: 0 2px 8px rgba(0,0,0,0.02);
}

.item-content {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.item-name {
  font-weight: 500;
  color: #2b2623;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.status-dot.enabled {
  background-color: #4caf50;
}

.status-dot.disabled {
  background-color: #9e9e9e;
}

.empty-list {
  padding: 2rem 1rem;
  text-align: center;
  color: #7a7067;
  font-size: 0.95rem;
}

.skill-details {
  background: #fff;
  border: 1px solid #e8e0d5;
  border-radius: 12px;
  padding: 2rem;
  box-shadow: 0 2px 12px rgba(0,0,0,0.03);
}

.details-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 2rem;
  padding-bottom: 1rem;
  border-bottom: 1px solid #f0ece5;
}

.details-header h2 {
  margin: 0;
  font-size: 1.5rem;
  color: #2b2623;
}

.details-content {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.field-group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.field-group label {
  font-size: 0.9rem;
  font-weight: 600;
  color: #5f5752;
}

.desc-text {
  margin: 0;
  color: #2b2623;
  line-height: 1.6;
}

.info-text {
  margin: 0;
  color: #5f5752;
  font-size: 0.95rem;
}

.actions-footer {
  margin-top: 1rem;
  display: flex;
  justify-content: flex-start;
}

.no-selection {
  display: flex;
  justify-content: center;
  align-items: center;
  height: 200px;
  color: #7a7067;
  border: 2px dashed #e5dbce;
  border-radius: 12px;
}

.btn {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.5rem 1rem;
  border-radius: 6px;
  font-size: 0.9rem;
  cursor: pointer;
  transition: all 0.2s;
}

.primary-btn {
  background: #2b2623;
  color: #fff;
  border: none;
}

.primary-btn:hover {
  background: #403a35;
}

.outline-btn {
  background: transparent;
  color: #5f5752;
  border: 1px solid #c8bcae;
}

.outline-btn:hover {
  background: #f0ece5;
  color: #2b2623;
}

.toggle-switch {
  position: relative;
  display: inline-block;
  width: 44px;
  height: 24px;
}

.toggle-switch input {
  opacity: 0;
  width: 0;
  height: 0;
}

.slider {
  position: absolute;
  cursor: pointer;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: #c8bcae;
  transition: .3s;
  border-radius: 24px;
}

.slider:before {
  position: absolute;
  content: "";
  height: 18px;
  width: 18px;
  left: 3px;
  bottom: 3px;
  background-color: white;
  transition: .3s;
  border-radius: 50%;
}

input:checked + .slider {
  background-color: #2b2623;
}

input:focus + .slider {
  box-shadow: 0 0 1px #2b2623;
}

input:checked + .slider:before {
  transform: translateX(20px);
}
</style>
