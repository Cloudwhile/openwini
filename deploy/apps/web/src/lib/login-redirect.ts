const STORAGE_KEY = "login_redirect"

export function loginRedirect(value: string | null): string | null {
  if (!value?.startsWith("/")) return null
  try {
    const target = new URL(value, window.location.origin)
    if (target.origin !== window.location.origin || target.pathname === "/login") return null
    return target.pathname + target.search + target.hash
  } catch {
    return null
  }
}

export function rememberLoginRedirect(value: string | null) {
  try {
    const target = loginRedirect(value)
    if (target) sessionStorage.setItem(STORAGE_KEY, target)
  } catch {
    // 浏览器禁用存储时仍可正常登录。
  }
}

export function clearLoginRedirect() {
  try {
    sessionStorage.removeItem(STORAGE_KEY)
  } catch {
    // 浏览器禁用存储时无需处理。
  }
}

export function takeLoginRedirect(): string | null {
  try {
    const target = loginRedirect(sessionStorage.getItem(STORAGE_KEY))
    sessionStorage.removeItem(STORAGE_KEY)
    return target
  } catch {
    return null
  }
}
