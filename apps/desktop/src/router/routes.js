export const routes = [
  { path: '/chat', key: 'chat', label: '聊天' },
  { path: '/settings', key: 'settings', label: '配置' },
  { path: '/memory', key: 'memory', label: '记忆' }
]

const routePaths = new Set(routes.map((route) => route.path))

export function normalizePath(path) {
  if (!path) return '/memory'
  const normalized = path.startsWith('/') ? path : `/${path}`
  return routePaths.has(normalized) ? normalized : '/memory'
}

export function pathFromHash(hashValue) {
  if (!hashValue) return '/memory'
  return normalizePath(hashValue.replace(/^#/, ''))
}
