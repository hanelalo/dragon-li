<script setup>
import { ref, onMounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import McpList from '../components/mcp/McpList.vue'
import McpEditor from '../components/mcp/McpEditor.vue'

const mcps = ref([])
const activeMcpId = ref('')
const errorMsg = ref('')
const isTesting = ref(false)

const activeMcp = computed(() => {
  return mcps.value.find(p => p.id === activeMcpId.value)
})

const hasSelection = computed(() => !!activeMcp.value)

async function loadMcps() {
  try {
    errorMsg.value = ''
    const res = await invoke('mcp_connector_list')
    if (res.ok) {
      mcps.value = res.data.connectors
      if (mcps.value.length > 0 && !activeMcpId.value) {
        activeMcpId.value = mcps.value[0].id
      }
    } else {
      errorMsg.value = res.error?.message || 'Failed to load MCP connectors'
    }
  } catch (err) {
    errorMsg.value = String(err)
  }
}

function handleSelectMcp(id) {
  activeMcpId.value = id
}

function handleAddMcp() {
  const newId = `mcp_${Date.now()}`
  const newMcp = {
    id: newId,
    name: 'New MCP Server',
    mcp_type: 'stdio',
    endpoint: '',
    status: 'configured',
    enabled: false,
    allowed_domains_json: null,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString()
  }
  
  // Optimistically add to UI
  mcps.value.push(newMcp)
  activeMcpId.value = newId
  
  // Create in DB
  invoke('mcp_connector_create', { connector: newMcp }).catch(err => {
    errorMsg.value = `Failed to create MCP connector: ${err}`
    // Revert on error
    mcps.value = mcps.value.filter(p => p.id !== newId)
    if (activeMcpId.value === newId) {
      activeMcpId.value = mcps.value.length > 0 ? mcps.value[0].id : ''
    }
  })
}

async function handleSaveMcp(updatedMcp) {
  try {
    errorMsg.value = ''
    updatedMcp.updated_at = new Date().toISOString()
    
    const res = await invoke('mcp_connector_update', {
      id: updatedMcp.id,
      name: updatedMcp.name,
      mcpType: updatedMcp.mcp_type,
      endpoint: updatedMcp.endpoint,
      status: updatedMcp.status,
      allowedDomainsJson: updatedMcp.allowed_domains_json,
      enabled: updatedMcp.enabled,
      updatedAt: updatedMcp.updated_at
    })
    
    if (res.ok) {
      // Update local state
      const idx = mcps.value.findIndex(p => p.id === updatedMcp.id)
      if (idx !== -1) {
        mcps.value[idx] = updatedMcp
      }
    } else {
      errorMsg.value = res.error?.message || 'Failed to update MCP connector'
    }
  } catch (err) {
    errorMsg.value = String(err)
  }
}

async function handleDeleteMcp(id) {
  try {
    errorMsg.value = ''
    const res = await invoke('mcp_connector_delete', { id })
    if (res.ok) {
      mcps.value = mcps.value.filter(p => p.id !== id)
      if (activeMcpId.value === id) {
        activeMcpId.value = mcps.value.length > 0 ? mcps.value[0].id : ''
      }
    } else {
      errorMsg.value = res.error?.message || 'Failed to delete MCP connector'
    }
  } catch (err) {
    errorMsg.value = String(err)
  }
}

function goBack() {
  window.location.hash = '/chat'
}

onMounted(() => {
  loadMcps()
})
</script>

<template>
  <section class="mcp-page">
    <header class="page-header">
      <div class="header-left">
        <button class="back-btn" @click="goBack" title="返回">
          <svg viewBox="0 0 24 24" width="20" height="20" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round">
            <line x1="19" y1="12" x2="5" y2="12"></line>
            <polyline points="12 19 5 12 12 5"></polyline>
          </svg>
        </button>
        <div>
          <h1>MCP 连接器</h1>
          <p>管理外部 Model Context Protocol (MCP) 服务器配置</p>
        </div>
      </div>
    </header>

    <div v-if="errorMsg" class="error-banner">
      {{ errorMsg }}
    </div>

    <div class="layout">
      <div class="sidebar">
        <McpList 
          :mcps="mcps" 
          :activeMcpId="activeMcpId"
          @select="handleSelectMcp"
          @add="handleAddMcp"
        />
      </div>
      
      <div class="editor-area">
        <McpEditor 
          v-if="hasSelection"
          :mcp="activeMcp"
          @save="handleSaveMcp"
          @delete="handleDeleteMcp"
        />
        <div v-else class="no-selection">
          <p>请选择一个 MCP 连接器或新建</p>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.mcp-page {
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
</style>