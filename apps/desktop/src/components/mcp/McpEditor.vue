<script setup>
import { ref, watch, computed } from 'vue'

const props = defineProps({
  mcp: {
    type: Object,
    required: true
  }
})

const emit = defineEmits(['save', 'delete', 'test'])

// Local reactive copy of the mcp and its config
const localMcp = ref(JSON.parse(JSON.stringify(props.mcp)))
if (!localMcp.value._config) {
  localMcp.value._config = { enabled: false, command: '', args: [], env: {}, url: '', headers: {} }
}

const showDeleteConfirm = ref(false)

// We want to handle args array as a string for easy text input
const argsString = computed({
  get: () => localMcp.value._config.args ? localMcp.value._config.args.join(' ') : '',
  set: (val) => {
    // A simple split by space. For more robust arg parsing, you might need a library,
    // but this is enough for MVP.
    localMcp.value._config.args = val.split(' ').filter(v => v.trim() !== '')
  }
})

const envString = computed({
  get: () => {
    const env = localMcp.value._config.env || {}
    return Object.entries(env).map(([k, v]) => `${k}=${v}`).join('\n')
  },
  set: (val) => {
    const env = {}
    val.split('\n').forEach(line => {
      const idx = line.indexOf('=')
      if (idx > 0) {
        const k = line.substring(0, idx).trim()
        const v = line.substring(idx + 1).trim()
        if (k) env[k] = v
      }
    })
    localMcp.value._config.env = env
  }
})

const headersString = computed({
  get: () => {
    const headers = localMcp.value._config.headers || {}
    return Object.entries(headers).map(([k, v]) => `${k}=${v}`).join('\n')
  },
  set: (val) => {
    const headers = {}
    val.split('\n').forEach(line => {
      const idx = line.indexOf('=')
      if (idx > 0) {
        const k = line.substring(0, idx).trim()
        const v = line.substring(idx + 1).trim()
        if (k) headers[k] = v
      }
    })
    localMcp.value._config.headers = headers
  }
})

watch(() => props.mcp, (newVal) => {
  localMcp.value = JSON.parse(JSON.stringify(newVal))
  if (!localMcp.value._config) {
    localMcp.value._config = { enabled: false, command: '', args: [], env: {}, url: '', headers: {} }
  }
  showDeleteConfirm.value = false
}, { deep: true })

function handleSave() {
  emit('save', JSON.parse(JSON.stringify(localMcp.value)))
}

function handleDelete() {
  showDeleteConfirm.value = true
}

function confirmDelete() {
  emit('delete', localMcp.value.id)
  showDeleteConfirm.value = false
}

function cancelDelete() {
  showDeleteConfirm.value = false
}

function handleTest() {
  emit('test', JSON.parse(JSON.stringify(localMcp.value)))
}
</script>

<template>
  <div class="mcp-editor">
    <div class="header">
      <h2>编辑 MCP Server</h2>
      <div class="actions">
        <button type="button" class="test-btn" @click="handleTest">测试连接</button>
        <button type="button" class="delete-btn" @click="handleDelete">删除</button>
      </div>
    </div>

    <div v-if="showDeleteConfirm" class="delete-confirm">
      <p>⚠️ 确认要删除该 MCP Server 吗？此操作无法撤销。</p>
      <div class="confirm-actions">
        <button type="button" class="cancel" @click="cancelDelete">取消</button>
        <button type="button" class="confirm" @click="confirmDelete">确认删除</button>
      </div>
    </div>

    <form class="form" @submit.prevent="handleSave">
      <div class="form-group">
        <label>名称</label>
        <input v-model="localMcp.name" type="text" required placeholder="例如: 本地文件系统 / SQLite" />
      </div>

      <div class="form-group row">
        <div class="col">
          <label>传输方式 (Transport)</label>
          <select v-model="localMcp.mcp_type" required>
            <option value="stdio">Stdio</option>
            <option value="sse">SSE</option>
            <option value="streamable_http">Streamable HTTP</option>
          </select>
        </div>
        <div class="col">
          <label>状态</label>
          <div class="toggle">
            <input type="checkbox" id="enabled-toggle" v-model="localMcp._config.enabled" />
            <label for="enabled-toggle">{{ localMcp._config.enabled ? '已启用' : '已禁用' }}</label>
          </div>
        </div>
      </div>

      <template v-if="localMcp.mcp_type === 'stdio'">
        <div class="form-group">
          <label>执行命令 (Command)</label>
          <input 
            v-model="localMcp._config.command" 
            type="text" 
            required 
            placeholder="例如: npx / uvx / node" 
          />
        </div>
        <div class="form-group">
          <label>参数 (Args)</label>
          <input 
            v-model="argsString" 
            type="text" 
            placeholder="例如: -y @modelcontextprotocol/server-sqlite --db-path ./test.db" 
          />
        </div>
        <div class="form-group">
          <label>环境变量 (Env)</label>
          <textarea 
            v-model="envString" 
            placeholder="KEY=VALUE&#10;TOKEN=abc"
            rows="3"
          ></textarea>
        </div>
      </template>

      <template v-else-if="localMcp.mcp_type === 'sse' || localMcp.mcp_type === 'streamable_http'">
        <div class="form-group">
          <label>{{ localMcp.mcp_type === 'sse' ? 'SSE URL' : 'HTTP URL' }}</label>
          <input 
            v-model="localMcp._config.url" 
            type="url" 
            required 
            :placeholder="localMcp.mcp_type === 'sse' ? '例如: http://localhost:8000/sse' : '例如: http://localhost:8000/mcp'" 
          />
        </div>
        <div class="form-group">
          <label>自定义请求头 (Headers)</label>
          <textarea 
            v-model="headersString" 
            placeholder="Authorization=Bearer token&#10;Custom-Header=value"
            rows="3"
          ></textarea>
        </div>
      </template>

      <div class="form-actions">
        <button type="submit" class="save-btn">保存更改</button>
      </div>
    </form>

    <div v-if="localMcp._status && localMcp._status.status === 'loading'" class="capabilities-section">
      <div class="capabilities-header">
        <h3>服务器能力 (Capabilities)</h3>
        <span class="badge loading">正在加载...</span>
      </div>
      <div class="empty-state">
        <p>正在连接并获取 MCP Server 支持的能力，请稍候...</p>
      </div>
    </div>

    <div v-else-if="localMcp._status && localMcp._status.status === 'connected'" class="capabilities-section">
      <div class="capabilities-header">
        <h3>服务器能力 (Capabilities)</h3>
        <span class="badge connected">已连接</span>
      </div>
      
      <div class="tools-list" v-if="localMcp._status.tools && localMcp._status.tools.length > 0">
        <h4>提供的工具 ({{ localMcp._status.tools.length }})</h4>
        <ul>
          <li v-for="tool in localMcp._status.tools" :key="tool.name">
            <div class="tool-name">{{ tool.name }}</div>
            <div class="tool-desc" v-if="tool.description">{{ tool.description }}</div>
          </li>
        </ul>
      </div>
      <div v-else class="empty-state">
        该服务器未提供任何工具。
      </div>
    </div>
    <div v-else-if="localMcp._status && localMcp._status.status === 'error'" class="capabilities-section">
      <div class="capabilities-header">
        <h3>服务器能力 (Capabilities)</h3>
        <span class="badge error">连接失败</span>
      </div>
      <div class="empty-state">
        <p>无法连接到该 MCP Server，请检查配置或重试。</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mcp-editor {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
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

.actions {
  display: flex;
  gap: 0.5rem;
}

button {
  border: 1px solid #c8b9a7;
  background: #f2ebe1;
  border-radius: 6px;
  padding: 0.4rem 0.8rem;
  font-size: 0.9rem;
  cursor: pointer;
}

.test-btn {
  background: #e3f2fd;
  border-color: #bbdefb;
  color: #1565c0;
}

.delete-btn {
  background: #fff;
  color: #dc3545;
  border: 1px solid #dc3545;
}
.delete-btn:hover {
  background: #dc3545;
  color: #fff;
}

.delete-confirm {
  background: #fff3cd;
  color: #856404;
  border: 1px solid #ffeeba;
  border-radius: 8px;
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
.delete-confirm p { margin: 0; }
.confirm-actions {
  display: flex;
  gap: 1rem;
  justify-content: flex-end;
}
.confirm-actions button {
  padding: 0.4rem 1rem;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-weight: 500;
}
.confirm-actions .cancel {
  background: #e2e3e5;
  color: #383d41;
}
.confirm-actions .confirm {
  background: #dc3545;
  color: white;
}

.form {
  display: flex;
  flex-direction: column;
  gap: 1.2rem;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.form-group.row {
  flex-direction: row;
  gap: 1rem;
}

.col {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

label {
  font-size: 0.9rem;
  font-weight: 500;
  color: #5f5953;
}

input[type="text"],
input[type="url"],
select,
textarea {
  border: 1px solid #d8cdbd;
  border-radius: 6px;
  background: #fffdf9;
  font: inherit;
  width: 100%;
  box-sizing: border-box;
}

input[type="text"],
input[type="url"],
textarea {
  padding: 0.6rem;
}

textarea {
  resize: vertical;
  font-family: monospace;
}

.toggle {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  height: 42px; /* match input height */
}

.toggle input {
  width: 1.2rem;
  height: 1.2rem;
}

.form-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 1rem;
}

.save-btn {
  background: #2d6a4f;
  color: white;
  border: none;
  padding: 0.6rem 1.5rem;
  font-weight: 500;
}

.capabilities-section {
  margin-top: 2rem;
  padding-top: 2rem;
  border-top: 1px solid #e5dbce;
}

.capabilities-header {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 1.5rem;
}

.capabilities-header h3 {
  margin: 0;
  font-size: 1.1rem;
  color: #2f2b28;
}

.tools-list h4 {
  font-size: 0.9rem;
  color: #746a62;
  margin: 0 0 1rem 0;
}

.tools-list ul {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
}

.tools-list li {
  background: #fdfaf5;
  border: 1px solid #d8cdbd;
  border-radius: 8px;
  padding: 1rem;
}

.tool-name {
  font-family: monospace;
  font-weight: 600;
  color: #2d6a4f;
  margin-bottom: 0.4rem;
}

.tool-desc {
  font-size: 0.85rem;
  color: #5f5953;
  line-height: 1.4;
}

.empty-state {
  color: #746a62;
  font-size: 0.9rem;
  font-style: italic;
}

.badge {
  font-size: 0.75rem;
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
}

.badge.enabled {
  background: #e9ecef;
  color: #495057;
}

.badge.loading {
  background: #fff3cd;
  color: #856404;
}

.badge.connected {
  background: #d4edda;
  color: #155724;
}

.badge.error {
  background: #f8d7da;
  color: #721c24;
}
</style>