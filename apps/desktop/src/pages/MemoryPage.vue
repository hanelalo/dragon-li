<script setup>
import { computed, onMounted, onUnmounted, reactive, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { appState } from '../state/appState'

const sessionId = computed({
  get: () => appState.runtime.activeSessionId,
  set: (value) => {
    appState.runtime.activeSessionId = value
  }
})

const extractInput = ref('我喜欢简洁的代码风格。下一个任务是补齐集成测试。')
const selectedCandidateId = ref('')
const selectedMemoryId = ref('')
const selectedMemory = ref(null)

const loading = reactive({
  candidates: false,
  longTerm: false,
  detail: false,
  extract: false,
  review: false
})

const errors = reactive({
  candidates: '',
  longTerm: '',
  detail: '',
  extract: '',
  review: ''
})

const filters = reactive({
  type: '',
  status: '',
  tags: '',
  minConfidence: 0.6,
  query: ''
})

const candidates = ref([])
const longTermItems = ref([])

const selectedCandidate = computed(() =>
  candidates.value.find((candidate) => candidate.id === selectedCandidateId.value) || null
)

function handleMemoryCandidatesRefreshed() {
  loadCandidates()
}

async function call(name, payload = {}) {
  return invoke(name, payload)
}

async function loadCandidates() {
  loading.candidates = true
  errors.candidates = ''
  try {
    const result = await call('memory_list_candidates', {
      sessionId: null, // Load candidates from all sessions
      status: 'pending' // Only show pending candidates for review
    })
    candidates.value = result?.data?.candidates || []
    if (!selectedCandidateId.value && candidates.value.length > 0) {
      selectedCandidateId.value = candidates.value[0].id
    }
  } catch (error) {
    errors.candidates = String(error)
  } finally {
    loading.candidates = false
  }
}

async function loadLongTerm() {
  loading.longTerm = true
  errors.longTerm = ''
  try {
    const result = await call('memory_list_long_term', {
      candidateType: filters.type || null,
      status: filters.status || null,
      minConfidence: Number(filters.minConfidence),
      tagsCsv: filters.tags || null,
      limit: 50
    })
    const items = result?.data?.items || []
    if (filters.query.trim()) {
      const query = filters.query.trim().toLowerCase()
      longTermItems.value = items.filter(
        (item) =>
          item.memory_id.toLowerCase().includes(query) ||
          item.tags.join(' ').toLowerCase().includes(query) ||
          item.candidate_type.toLowerCase().includes(query)
      )
    } else {
      longTermItems.value = items
    }
    if (!selectedMemoryId.value && longTermItems.value.length > 0) {
      selectedMemoryId.value = longTermItems.value[0].memory_id
      await loadMemoryDetail(selectedMemoryId.value)
    }
  } catch (error) {
    errors.longTerm = String(error)
  } finally {
    loading.longTerm = false
  }
}

async function loadMemoryDetail(memoryId) {
  if (!memoryId) return
  loading.detail = true
  errors.detail = ''
  try {
    const result = await call('memory_read', { memoryId })
    selectedMemory.value = result?.data?.memory || null
  } catch (error) {
    selectedMemory.value = null
    errors.detail = String(error)
  } finally {
    loading.detail = false
  }
}

async function extractCandidates() {
  loading.extract = true
  errors.extract = ''
  try {
    const sourceMessageId = `m_extract_${Date.now()}`
    await call('message_create', {
      message: {
        id: sourceMessageId,
        session_id: sessionId.value,
        role: 'assistant',
        content_md: extractInput.value,
        provider: null,
        model: null,
        tokens_in: null,
        tokens_out: null,
        latency_ms: null,
        parent_message_id: null,
        status: 'ok',
        error_code: null,
        error_message: null,
        retryable: null,
        created_at: new Date().toISOString()
      }
    }).catch(() => {})
    await call('memory_extract_candidates', {
      input: {
        session_id: sessionId.value,
        source_message_id: sourceMessageId,
        content: extractInput.value
      }
    })
    await loadCandidates()
  } catch (error) {
    errors.extract = String(error)
  } finally {
    loading.extract = false
  }
}

async function reviewCandidate(action) {
  if (!selectedCandidateId.value) return
  
  const payload = {
    candidate_id: selectedCandidateId.value,
    action
  }
  
  if (action === 'merge') {
    if (!selectedMemoryId.value) {
      errors.review = 'Please select a target long-term memory on the right first'
      return
    }
    payload.merge_target_id = selectedMemoryId.value
  }

  loading.review = true
  errors.review = ''
  try {
    const result = await call('memory_review_candidate', { input: payload })
    const memory = result?.data?.memory
    
    // Decrement the unreviewed memory badge count on successful review
    if (appState.memory.unreviewedCount > 0) {
      appState.memory.unreviewedCount--
    }

    if (action === 'reject') {
      // Clear selection after reject to refresh view properly
      selectedCandidateId.value = ''
    } else if (memory?.memory_id) {
      selectedMemoryId.value = memory.memory_id
      await loadMemoryDetail(memory.memory_id)
    }
    await Promise.all([loadCandidates(), loadLongTerm()])
  } catch (error) {
    errors.review = String(error)
  } finally {
    loading.review = false
  }
}

async function removeMemory() {
  if (!selectedMemoryId.value) return
  try {
    await call('memory_soft_delete', {
      memoryId: selectedMemoryId.value,
      deletedAt: new Date().toISOString()
    })
    selectedMemory.value = null
    selectedMemoryId.value = ''
    await loadLongTerm()
  } catch (error) {
    errors.detail = String(error)
  }
}

async function restoreMemory() {
  if (!selectedMemoryId.value) return
  try {
    await call('memory_restore', { memoryId: selectedMemoryId.value })
    await loadLongTerm()
    await loadMemoryDetail(selectedMemoryId.value)
  } catch (error) {
    errors.detail = String(error)
  }
}

function statusClass(status) {
  return `tag status-${status || 'na'}`
}

onMounted(async () => {
  window.addEventListener('memory-candidates-refreshed', handleMemoryCandidatesRefreshed)
  await Promise.all([loadCandidates(), loadLongTerm()])
})

onUnmounted(() => {
  window.removeEventListener('memory-candidates-refreshed', handleMemoryCandidatesRefreshed)
})
</script>

<template>
  <div class="memory-page">
    <header class="hero">
      <h1>Memory Center</h1>
      <p>Review candidates and manage long-term memory with filters and detail view.</p>
    </header>

    <section class="toolbar">
      <div class="field">
        <label>Type</label>
        <select v-model="filters.type">
          <option value="">All</option>
          <option value="fact">fact</option>
          <option value="preference">preference</option>
          <option value="constraint">constraint</option>
          <option value="project">project</option>
          <option value="task">task</option>
        </select>
      </div>
      <div class="field">
        <label>Status</label>
        <select v-model="filters.status">
          <option value="">All</option>
          <option value="approved">approved</option>
          <option value="pending">pending</option>
          <option value="rejected">rejected</option>
          <option value="conflicted">conflicted</option>
        </select>
      </div>
      <div class="field">
        <label>Min Confidence</label>
        <input v-model.number="filters.minConfidence" min="0" max="1" step="0.01" type="number" />
      </div>
      <div class="field wide">
        <label>Tags (comma)</label>
        <input v-model="filters.tags" placeholder="rust,tauri" />
      </div>
      <div class="field wide">
        <label>Search keyword</label>
        <input v-model="filters.query" placeholder="memory id / tags / type" />
      </div>
      <button class="primary" @click="loadLongTerm">Apply Filters</button>
    </section>

    <section class="extract-box" v-if="false">
      <label>Extract Candidate Input</label>
      <textarea v-model="extractInput" rows="3" />
      <div class="row">
        <button class="primary" :disabled="loading.extract" @click="extractCandidates">
          {{ loading.extract ? 'Extracting...' : 'Extract Candidates' }}
        </button>
        <span v-if="errors.extract" class="error">{{ errors.extract }}</span>
      </div>
    </section>

    <section class="layout">
      <article class="panel">
        <h2>Candidate Review</h2>
        <p v-if="loading.candidates" class="state">Loading candidates...</p>
        <p v-else-if="errors.candidates" class="error">{{ errors.candidates }}</p>
        <p v-else-if="candidates.length === 0" class="state">No candidates yet.</p>
        <ul v-else class="list">
          <li
            v-for="item in candidates"
            :key="item.id"
            :class="{ active: selectedCandidateId === item.id }"
            @click="selectedCandidateId = item.id"
          >
            <div class="line">
              <strong>{{ item.candidate_type }}</strong>
              <span :class="statusClass(item.status)">{{ item.status }}</span>
            </div>
            <p>{{ item.summary }}</p>
            <small>conf: {{ item.confidence?.toFixed(2) }}</small>
          </li>
        </ul>
        <div class="actions">
          <button :disabled="!selectedCandidateId || loading.review" @click="reviewCandidate('approve')">Approve</button>
          <button :disabled="!selectedCandidateId || loading.review" @click="reviewCandidate('reject')">Reject</button>
          <button :disabled="!selectedCandidateId || !selectedMemoryId || loading.review" @click="reviewCandidate('merge')" title="Select a Long-Term memory on the right first">Merge into Selected</button>
        </div>
        <p v-if="errors.review" class="error">{{ errors.review }}</p>
        <pre v-if="selectedCandidate" class="json">{{ JSON.stringify(selectedCandidate, null, 2) }}</pre>
      </article>

      <article class="panel">
        <h2>Long-Term Memory</h2>
        <p v-if="loading.longTerm" class="state">Loading long-term memory...</p>
        <p v-else-if="errors.longTerm" class="error">{{ errors.longTerm }}</p>
        <p v-else-if="longTermItems.length === 0" class="state">No long-term memory matched filters.</p>
        <ul v-else class="list">
          <li
            v-for="item in longTermItems"
            :key="item.memory_id"
            :class="{ active: selectedMemoryId === item.memory_id }"
            @click="selectedMemoryId = item.memory_id; loadMemoryDetail(item.memory_id)"
          >
            <div class="line">
              <strong>{{ item.candidate_type }}</strong>
              <span :class="statusClass(item.status)">{{ item.status }}</span>
            </div>
            <p>{{ item.summary }}</p>
            <small>conf: {{ item.confidence?.toFixed(2) }}</small>
          </li>
        </ul>
      </article>

      <article class="panel detail">
        <h2>Memory Detail</h2>
        <p v-if="loading.detail" class="state">Loading detail...</p>
        <p v-else-if="errors.detail" class="error">{{ errors.detail }}</p>
        <p v-else-if="!selectedMemory" class="state">Select a memory from the list.</p>
        <template v-else>
          <div class="line">
            <strong>{{ selectedMemory.memory_id }}</strong>
            <span :class="statusClass(selectedMemory.status)">{{ selectedMemory.status }}</span>
          </div>
          <p>{{ selectedMemory.summary }}</p>
          <small>tags: {{ selectedMemory.tags.join(', ') || '-' }}</small>
          <div class="actions">
            <button @click="removeMemory">Soft Delete</button>
            <button @click="restoreMemory">Restore</button>
          </div>
          <pre class="markdown">{{ selectedMemory.markdown }}</pre>
        </template>
      </article>
    </section>
  </div>
</template>

<style scoped>
.memory-page {
  display: grid;
  gap: 0.9rem;
}

.hero h1 {
  margin: 0;
  font-size: clamp(1.6rem, 2.8vw, 2.4rem);
}

.hero p {
  margin: 0.3rem 0 1rem;
  color: #6f6460;
}

.toolbar,
.extract-box,
.panel {
  background: #fffdf7;
  border: 1px solid #d6ccbf;
  border-radius: 14px;
  box-shadow: 0 10px 26px rgba(39, 31, 27, 0.08);
}

.toolbar {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
  gap: 0.7rem;
  padding: 0.9rem;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.field.wide {
  grid-column: span 2;
}

label {
  font-size: 0.76rem;
  color: #6f6460;
}

input,
select,
textarea,
button {
  font: inherit;
}

input,
textarea {
  border: 1px solid #d6ccbf;
  border-radius: 10px;
  padding: 0.45rem 0.55rem;
  background: #fff;
}

select {
  border: 1px solid #d6ccbf;
  border-radius: 10px;
  background: #fff;
}

button {
  border: 1px solid #b7b0a6;
  border-radius: 10px;
  padding: 0.5rem 0.75rem;
  background: #f6f3ee;
  cursor: pointer;
}

button.primary {
  background: #2d6a4f;
  border-color: #2d6a4f;
  color: #fff;
}

button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.extract-box {
  padding: 0.9rem;
}

.row {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  margin-top: 0.5rem;
}

.layout {
  display: grid;
  grid-template-columns: 1fr 1fr 1.2fr;
  gap: 0.9rem;
}

.panel {
  padding: 0.9rem;
}

.panel h2 {
  margin: 0 0 0.45rem;
  font-size: 1rem;
}

.list {
  list-style: none;
  margin: 0;
  padding: 0;
  max-height: 360px;
  overflow: auto;
  border: 1px solid #d6ccbf;
  border-radius: 10px;
}

.list li {
  padding: 0.6rem;
  border-bottom: 1px solid #ece4d9;
  cursor: pointer;
}

.list li:last-child {
  border-bottom: 0;
}

.list li.active {
  background: #e6f2eb;
}

.line {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 0.6rem;
}

.tag {
  padding: 0.1rem 0.45rem;
  border-radius: 999px;
  font-size: 0.72rem;
  border: 1px solid #d6ccbf;
  background: #f8f3ea;
}

.status-approved {
  color: #14532d;
}

.status-pending {
  color: #92400e;
}

.status-rejected {
  color: #7f1d1d;
}

.status-conflicted {
  color: #5b21b6;
}

.actions {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
  margin-top: 0.65rem;
}

.state {
  color: #6f6460;
}

.error {
  color: #9a3412;
}

.json,
.markdown {
  margin-top: 0.6rem;
  background: #1f2937;
  color: #d6e3ff;
  border-radius: 10px;
  padding: 0.65rem;
  overflow: auto;
  max-height: 300px;
}

@media (max-width: 1100px) {
  .layout {
    grid-template-columns: 1fr;
  }

  .field.wide {
    grid-column: auto;
  }
}
</style>
