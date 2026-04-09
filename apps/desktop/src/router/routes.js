export const routes = [
  { path: '/chat', key: 'chat', label: 'Chat' },
  { path: '/settings', key: 'settings', label: 'Settings' },
  { path: '/memory', key: 'memory', label: 'Memory' },
  { path: '/mcp', key: 'mcp', label: 'MCP' },
  { path: '/skill', key: 'skill', label: 'Skill' }
]

const routePaths = new Set(routes.map((route) => route.path))

export function normalizePath(path) {
  if (!path) return '/chat'
  const normalized = path.startsWith('/') ? path : `/${path}`
  return routePaths.has(normalized) ? normalized : '/chat'
}

export function pathFromHash(hashValue) {
  if (!hashValue) return '/chat'
  return normalizePath(hashValue.replace(/^#/, ''))
}
