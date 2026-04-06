import { reactive } from 'vue'

export const appState = reactive({
  nav: {
    lastVisitedPath: '/chat'
  },
  runtime: {
    activeSessionId: '',
    activeProfileId: '',
    lastRequestId: '',
    lastError: ''
  },
  chat: {
    sessions: []
  },
  settings: {
    profiles: []
  },
  memory: {
    unreviewedCount: 0
  }
})
