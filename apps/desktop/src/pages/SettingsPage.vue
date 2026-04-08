<script setup>
import { computed, onMounted, ref, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

import { appState } from '../state/appState'
import ProfileList from '../components/settings/ProfileList.vue'
import ProfileEditor from '../components/settings/ProfileEditor.vue'

const profiles = ref([])
const activeEditorId = ref(null)
const activeProfile = computed(() => profiles.value.find(p => p.id === activeEditorId.value))
const errorMsg = ref('')
const testResult = ref(null) // { success: boolean, message: string }
const hasExternalChange = ref(false)
const toolsSaveSuccess = ref(false)
const activeTab = ref('models')

onMounted(async () => {
  await loadConfig()
  checkExternalChange()
  
  // 监听窗口聚焦事件，重新检查外部变更
  window.addEventListener('focus', checkExternalChange)
})

onUnmounted(() => {
  window.removeEventListener('focus', checkExternalChange)
})

async function loadConfig() {
  try {
    const res = await invoke('config_get')
    if (res.ok) {
      profiles.value = res.data.config.profiles || []
      appState.settings.tools.braveSearchApiKey = res.data.config.tools?.brave_search_api_key || ''
      syncGlobalProfiles(profiles.value)
    } else {
      errorMsg.value = `加载配置失败: ${res.error?.code || res.error || '未知错误'}`
    }
  } catch (err) {
    errorMsg.value = String(err)
  }
}

async function checkExternalChange() {
  try {
    const res = await invoke('config_check_external_change')
    if (res.ok && res.data.external_changed) {
      hasExternalChange.value = true
    }
  } catch (err) {
    console.error('Check external change failed:', err)
  }
}

async function applyExternalChange(confirm) {
  try {
    const res = await invoke('config_apply_external_change', { confirm })
    if (res.ok) {
      profiles.value = res.data.config.profiles || []
      appState.settings.tools.braveSearchApiKey = res.data.config.tools?.brave_search_api_key || ''
      syncGlobalProfiles(profiles.value)
      hasExternalChange.value = false
      errorMsg.value = ''
    } else {
      errorMsg.value = `应用配置失败: ${res.error?.code || res.error || '未知错误'}`
    }
  } catch (err) {
    errorMsg.value = String(err)
  }
}

function syncGlobalProfiles(currentProfiles) {
  appState.settings.profiles = currentProfiles.map(p => ({
    id: p.id,
    name: p.name,
    enabled: p.enabled,
    default_model: p.default_model
  }))
  
  // Update active profile if it was deleted or disabled
  if (!currentProfiles.find(p => p.id === appState.runtime.activeProfileId && p.enabled)) {
    const firstEnabled = currentProfiles.find(p => p.enabled)
    appState.runtime.activeProfileId = firstEnabled ? firstEnabled.id : ''
  }
}

async function saveConfig(newProfiles) {
  try {
    const configPayload = {
      profiles: newProfiles,
      tools: {
        brave_search_api_key: appState.settings.tools.braveSearchApiKey || null
      }
    }
    const res = await invoke('config_save', { config: configPayload })
    if (res.ok) {
      profiles.value = res.data.config.profiles || []
      appState.settings.tools.braveSearchApiKey = res.data.config.tools?.brave_search_api_key || ''
      syncGlobalProfiles(profiles.value)
      errorMsg.value = ''
      return true
    } else {
      if (res.error?.code === 'CONFIG_RELOAD_REJECTED' || res.error === 'CONFIG_RELOAD_REJECTED') {
        hasExternalChange.value = true
        errorMsg.value = '检测到外部配置变更，请先处理冲突'
      } else {
        errorMsg.value = `保存失败: ${res.error?.code || res.error || '未知错误'}`
      }
      return false
    }
  } catch (err) {
    errorMsg.value = `保存异常: ${err}`
    return false
  }
}

function handleAddProfile() {
  const newId = `profile_${Date.now()}`
  const newProfile = {
    id: newId,
    name: 'New Profile',
    provider: 'openai',
    base_url: 'https://api.openai.com/v1',
    api_key: '',
    default_model: 'gpt-4o',
    enabled: true,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString()
  }
  profiles.value.push(newProfile)
  activeEditorId.value = newId
}

function handleSelectProfile(id) {
  activeEditorId.value = id
  testResult.value = null
  errorMsg.value = ''
}

async function handleSaveProfile(updatedProfile) {
  testResult.value = null // 清除测试结果
  updatedProfile.updated_at = new Date().toISOString()
  
  // Create a copy of profiles to try saving
  const newProfiles = [...profiles.value]
  const idx = newProfiles.findIndex(p => p.id === updatedProfile.id)
  if (idx !== -1) {
    newProfiles[idx] = updatedProfile
  } else {
    // shouldn't happen but just in case
    newProfiles.push(updatedProfile)
  }
  
  const success = await saveConfig(newProfiles)
  if (success) {
    // Only update local state if backend save succeeded
    profiles.value = newProfiles
  }
}

async function handleDeleteProfile(id) {
  const newProfiles = profiles.value.filter(p => p.id !== id)
  const success = await saveConfig(newProfiles)
  if (success) {
    profiles.value = newProfiles
    if (activeEditorId.value === id) {
      activeEditorId.value = null
    }
  }
}

async function handleTestProfile(profileToTest) {
  testResult.value = { success: null, message: '正在保存配置并测试连接...' }
  errorMsg.value = ''
  
  // 先保存当前编辑的内容，确保 chat_send 读取到最新配置
  await handleSaveProfile(profileToTest)
  if (errorMsg.value) {
    testResult.value = { success: false, message: '配置保存失败，无法测试' }
    return 
  }

  testResult.value = { success: null, message: '正在发起请求...' }

  const requestId = `test_req_${Date.now()}`
  const request = {
    profile_id: profileToTest.id,
    request_id: requestId,
    session_id: 'test_connection_session',
    model: profileToTest.default_model,
    prompt: {
      system: 'You are a connection tester. Respond with "ok".',
      runtime: '',
      memory: '',
      user: 'Return "ok"'
    },
    history: []
  }
  
  testResult.value = { success: null, message: '正在等待响应...' }

  let unlistenStream = null
  try {
    unlistenStream = await listen('chat_stream_event', (event) => {
      const payload = event.payload
      if (payload.request_id !== requestId) return

      if (payload.event.type === 'done') {
        testResult.value = { success: true, message: '测试成功: 连接正常' }
        if (unlistenStream) unlistenStream()
      } else if (payload.event.type === 'aborted') {
        testResult.value = { success: false, message: `测试失败 [${payload.event.code}]: ${payload.event.message}` }
        if (unlistenStream) unlistenStream()
      }
    })

    const res = await invoke('chat_send', { request })
    if (!res.ok) {
      // 提取 Tauri 返回的报错
      const errCode = res.error?.code || res.error || 'UNKNOWN'
      const errMsg = res.error?.message || ''
      testResult.value = { success: false, message: `测试失败 [${errCode}]: ${errMsg}` }
      if (unlistenStream) unlistenStream()
    }
    // 如果 res.ok 为 true，则继续等待流事件（done 或 aborted）来更新结果
  } catch (err) {
    testResult.value = { success: false, message: `测试异常: ${err.message || err}` }
    if (unlistenStream) unlistenStream()
  }
}

async function handleSaveTools() {
  const success = await saveConfig(profiles.value)
  if (success) {
    toolsSaveSuccess.value = true
    setTimeout(() => {
      toolsSaveSuccess.value = false
    }, 2000)
  }
}

function goBack() {
  window.location.hash = '/chat'
}
</script>

<template>
  <section class="settings-page">
    <header class="page-header">
      <div class="header-left">
        <button class="back-btn" @click="goBack" title="返回聊天">
          <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <line x1="19" y1="12" x2="5" y2="12"></line>
            <polyline points="12 19 5 12 12 5"></polyline>
          </svg>
        </button>
        <div>
          <h1>配置</h1>
          <p>管理大模型服务商与工具配置</p>
        </div>
      </div>
    </header>

    <div v-if="hasExternalChange" class="external-change-alert">
      <span>检测到配置文件在外部被修改。</span>
      <div class="actions">
        <button @click="applyExternalChange(true)">加载最新配置</button>
        <button @click="applyExternalChange(false)" class="overwrite-btn">强制覆盖</button>
      </div>
    </div>

    <div v-if="errorMsg" class="error-banner">
      {{ errorMsg }}
    </div>

    <div class="layout">
      <div class="sidebar">
        <div class="settings-nav">
          <div 
            class="nav-item" 
            :class="{ active: activeTab === 'models' }"
            @click="activeTab = 'models'"
          >
            模型配置
          </div>
          <div 
            class="nav-item" 
            :class="{ active: activeTab === 'tools' }"
            @click="activeTab = 'tools'"
          >
            工具配置
          </div>
        </div>

        <div class="profile-list-container" v-if="activeTab === 'models'">
          <ProfileList 
            :profiles="profiles"
            :activeProfileId="activeEditorId"
            @select="handleSelectProfile"
            @add="handleAddProfile"
          />
        </div>
      </div>
      
      <div class="editor-area">
        <template v-if="activeTab === 'models'">
          <ProfileEditor 
            v-if="activeProfile"
            :profile="activeProfile"
            @save="handleSaveProfile"
            @delete="handleDeleteProfile"
            @test="handleTestProfile"
          />
          <div v-else class="no-selection">
            <p>请选择一个 Profile 或新建</p>
          </div>
          
          <div v-if="testResult" :class="['test-result', testResult.success === true ? 'success' : testResult.success === false ? 'error' : 'loading']">
            <h4>测试结果</h4>
            <p>{{ testResult.message }}</p>
          </div>
        </template>

        <template v-if="activeTab === 'tools'">
          <div class="tools-config-card">
            <h3>全局工具配置</h3>
            <div class="form-group">
              <label>Brave Search API Key</label>
              <div class="input-with-button">
                <input 
                  type="password" 
                  v-model="appState.settings.tools.braveSearchApiKey" 
                  placeholder="用于开启网络搜索功能的 API Key" 
                />
                <button @click="handleSaveTools" class="save-tools-btn" :class="{ success: toolsSaveSuccess }">
                  {{ toolsSaveSuccess ? '已保存 ✓' : '保存配置' }}
                </button>
              </div>
            </div>
          </div>
        </template>
      </div>
    </div>
  </section>
</template>

<style scoped>
.settings-page {
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

.external-change-alert {
  background: #fff3cd;
  color: #856404;
  padding: 1rem;
  border-radius: 8px;
  border: 1px solid #ffeeba;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.external-change-alert button {
  margin-left: 0.5rem;
  padding: 0.4rem 0.8rem;
  border: 1px solid #ffeeba;
  border-radius: 4px;
  background: white;
  cursor: pointer;
}

.overwrite-btn {
  color: #dc3545;
}

.error-banner {
  background: #f8d7da;
  color: #721c24;
  padding: 1rem;
  border-radius: 8px;
  border: 1px solid #f5c6cb;
}

.tools-config-card {
  background: #fffdf9;
  border: 1px solid #e5dbce;
  border-radius: 8px;
  padding: 1.5rem;
}

.tools-config-card h3 {
  margin: 0 0 1rem 0;
  font-size: 1.1rem;
  color: #2b2623;
}

.tools-config-card .form-group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.tools-config-card label {
  font-size: 0.9rem;
  font-weight: 500;
  color: #5f5953;
}

.tools-config-card .input-with-button {
  display: flex;
  gap: 1rem;
}

.tools-config-card input {
  flex: 1;
  padding: 0.6rem;
  border: 1px solid #d8cdbd;
  border-radius: 6px;
  background: #fff;
  font: inherit;
}

.tools-config-card .save-tools-btn {
  background: #2d6a4f;
  color: white;
  border: none;
  padding: 0.6rem 1.5rem;
  border-radius: 6px;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.2s;
}

.tools-config-card .save-tools-btn:hover:not(.success) {
  opacity: 0.9;
}

.tools-config-card .save-tools-btn.success {
  background: #198754;
}

.layout {
  display: grid;
  grid-template-columns: 280px 1fr;
  gap: 2rem;
  flex: 1;
  min-height: 0;
}

.sidebar {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  overflow: hidden;
}

.settings-nav {
  display: flex;
  gap: 0.5rem;
  padding-bottom: 0.5rem;
  border-bottom: 1px solid #e5dbce;
}

.nav-item {
  padding: 0.4rem 0.8rem;
  border-radius: 6px;
  cursor: pointer;
  color: #5f5953;
  font-weight: 500;
  font-size: 0.95rem;
  transition: all 0.2s;
}

.nav-item:hover {
  background: #f5eee4;
}

.nav-item.active {
  background: #e5dbce;
  color: #2b2623;
}

.profile-list-container {
  overflow-y: auto;
  flex: 1;
}

.editor-area {
  overflow-y: auto;
  padding-right: 1rem;
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

.test-result {
  margin-top: 1.5rem;
  padding: 1rem;
  border-radius: 8px;
  border: 1px solid;
}

.test-result.success {
  background: #d4edda;
  border-color: #c3e6cb;
  color: #155724;
}

.test-result.error {
  background: #f8d7da;
  border-color: #f5c6cb;
  color: #721c24;
}

.test-result.loading {
  background: #e2e3e5;
  border-color: #d6d8db;
  color: #383d41;
}

.test-result h4 {
  margin: 0 0 0.5rem 0;
}

.test-result p {
  margin: 0;
  font-family: monospace;
  white-space: pre-wrap;
}
</style>
