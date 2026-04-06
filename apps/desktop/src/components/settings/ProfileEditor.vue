<script setup>
import { ref, watch } from 'vue'

const props = defineProps({
  profile: {
    type: Object,
    required: true
  }
})

const emit = defineEmits(['save', 'delete', 'test'])

// Local reactive copy of the profile
const localProfile = ref({ ...props.profile })
const isEditingApiKey = ref(!localProfile.value.api_key)
const showDeleteConfirm = ref(false)

watch(() => props.profile, (newVal) => {
  localProfile.value = { ...newVal }
  isEditingApiKey.value = !newVal.api_key
  showDeleteConfirm.value = false
})

function handleSave() {
  emit('save', { ...localProfile.value })
}

function handleDelete() {
  showDeleteConfirm.value = true
}

function confirmDelete() {
  emit('delete', localProfile.value.id)
  showDeleteConfirm.value = false
}

function cancelDelete() {
  showDeleteConfirm.value = false
}

function handleTest() {
  emit('test', { ...localProfile.value })
}
</script>

<template>
  <div class="profile-editor">
    <div class="header">
      <h2>编辑 Profile</h2>
      <div class="actions">
        <button type="button" class="test-btn" @click="handleTest">测试连接</button>
        <button type="button" class="delete-btn" @click="handleDelete">删除</button>
      </div>
    </div>

    <div v-if="showDeleteConfirm" class="delete-confirm">
      <p>⚠️ 确认要删除该 Profile 吗？此操作无法撤销。</p>
      <div class="confirm-actions">
        <button type="button" class="cancel" @click="cancelDelete">取消</button>
        <button type="button" class="confirm" @click="confirmDelete">确认删除</button>
      </div>
    </div>

    <form class="form" @submit.prevent="handleSave">
      <div class="form-group">
        <label>名称</label>
        <input v-model="localProfile.name" type="text" required placeholder="例如: 默认 OpenAI" />
      </div>

      <div class="form-group row">
        <div class="col">
          <label>Provider</label>
          <select v-model="localProfile.provider" required>
            <option value="openai">OpenAI</option>
            <option value="anthropic">Anthropic</option>
          </select>
        </div>
        <div class="col">
          <label>状态</label>
          <div class="toggle">
            <input type="checkbox" id="enabled-toggle" v-model="localProfile.enabled" />
            <label for="enabled-toggle">{{ localProfile.enabled ? '已启用' : '已禁用' }}</label>
          </div>
        </div>
      </div>

      <div class="form-group">
        <label>Base URL</label>
        <input 
          v-model="localProfile.base_url" 
          type="url" 
          required 
          placeholder="https://api.openai.com/v1" 
          pattern="^https://.*"
          title="Base URL 必须以 https:// 开头"
        />
      </div>

      <div class="form-group">
        <label>API Key</label>
        <div class="api-key-input">
          <input 
            v-if="isEditingApiKey"
            v-model="localProfile.api_key" 
            type="text" 
            required 
            placeholder="sk-..." 
          />
          <div v-else class="api-key-masked">
            <span>••••••••••••••••••••••••••••</span>
            <button type="button" @click="isEditingApiKey = true">修改</button>
          </div>
        </div>
      </div>

      <div class="form-group">
        <label>默认模型</label>
        <input v-model="localProfile.default_model" type="text" required placeholder="gpt-4o / claude-3-5-sonnet-20240620" />
      </div>

      <div class="form-actions">
        <button type="submit" class="save-btn">保存更改</button>
      </div>
    </form>
  </div>
</template>

<style scoped>
.profile-editor {
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
select {
  border: 1px solid #d8cdbd;
  border-radius: 6px;
  background: #fffdf9;
  font: inherit;
  width: 100%;
  box-sizing: border-box;
}

input[type="text"],
input[type="url"] {
  padding: 0.6rem;
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

.api-key-input {
  display: flex;
  gap: 0.5rem;
  align-items: center;
}

.api-key-masked {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  border: 1px solid #d8cdbd;
  border-radius: 6px;
  padding: 0.6rem;
  background: #f5eee4;
}

.api-key-masked span {
  color: #746a62;
  letter-spacing: 2px;
}

.api-key-masked button {
  padding: 0.2rem 0.6rem;
  font-size: 0.8rem;
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
</style>
