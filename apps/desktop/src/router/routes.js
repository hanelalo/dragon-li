export const routes = [
  { path: '/chat', key: 'chat', label: '聊天' },
  { path: '/settings', key: 'settings', label: '配置' },
  { path: '/memory', key: 'memory', label: '记忆' }
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
