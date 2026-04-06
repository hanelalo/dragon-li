import { reactive } from 'vue'

export const appState = reactive({
  nav: {
    lastVisitedPath: '/memory'
  },
  runtime: {
    activeSessionId: 's1',
    activeProfileId: '',
    lastRequestId: '',
    lastError: ''
  },
  chat: {
    sessions: []
  },
  settings: {
    profiles: []
  }
})
