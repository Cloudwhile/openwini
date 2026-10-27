/** 统一 API 客户端：自动附带 CSRF Token（来自 /api/auth/me），错误统一抛 ApiError */
import { getLang, t } from "@/i18n"

let csrfToken: string | null = null

export function setCsrfToken(token: string | null) {
  csrfToken = token
}

export class ApiError extends Error {
  status: number
  data: unknown
  constructor(status: number, message: string, data: unknown) {
    super(message)
    this.status = status
    this.data = data
  }
  private asObject(): Record<string, unknown> | null {
    const d = typeof this.data === "string" ? safeParse(this.data) : this.data
    return d && typeof d === "object" ? (d as Record<string, unknown>) : null
  }
  /** 后端返回的 {error} 字段；清单比对错误时为 JSON 字符串 */
  get differences(): string[] | null {
    const d = this.asObject()
    if (d && Array.isArray(d.differences)) return d.differences as string[]
    return null
  }
  /** 带错误码的错误（见 apiErrors 词典），如 manifest_missing */
  get code(): string | null {
    const d = this.asObject()
    return d && typeof d.code === "string" ? d.code : null
  }
  /** 错误码参数（owner/repo/branch 等） */
  get params(): Record<string, string> {
    const d = this.asObject()
    return d && d.params && typeof d.params === "object" ? (d.params as Record<string, string>) : {}
  }
}

function safeParse(text: string): unknown {
  try {
    return JSON.parse(text)
  } catch {
    return null
  }
}

/** 把任意错误转成当前语言的用户可读消息：优先按错误码查 apiErrors 词典，回退原始 message */
export function apiErrMessage(e: unknown): string {
  if (e instanceof ApiError) {
    const code = e.code
    const dict = (t as unknown as { apiErrors?: Record<string, unknown> }).apiErrors
    const tpl = code && dict ? dict[code] : undefined
    if (typeof tpl === "function") return (tpl as (p: Record<string, string>) => string)(e.params)
    if (typeof tpl === "string") return tpl
  }
  return e instanceof Error ? e.message : String(e)
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const method = (init.method ?? "GET").toUpperCase()
  const headers = new Headers(init.headers)
  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json")
  }
  if (csrfToken && method !== "GET" && method !== "HEAD") {
    headers.set("X-CSRF-Token", csrfToken)
  }
  let res: Response
  try {
    res = await fetch(path, { ...init, headers, credentials: "same-origin" })
  } catch {
    throw new ApiError(0, t.errors.network, null)
  }
  if (res.status === 204) return undefined as T
  const text = await res.text()
  let data: unknown
  try {
    data = text ? JSON.parse(text) : null
  } catch {
    data = text
  }
  if (!res.ok) {
    // 会话过期/未授权：清除本地会话状态并跳登录页（/api/auth/* 的 401 属正常探测，不跳）
    if (res.status === 401 && !path.startsWith("/api/auth/")) {
      setCsrfToken(null)
      if (!window.location.pathname.startsWith("/login")) {
        const current = window.location.pathname + window.location.search
        window.location.href = `/login?error=${encodeURIComponent(t.misc.sessionExpired)}&redirect=${encodeURIComponent(current)}`
      }
    }
    const msg =
      (data && typeof data === "object" && "error" in (data as Record<string, unknown>)
        ? String((data as Record<string, unknown>).error)
        : typeof data === "string" && data
          ? data
          : `HTTP ${res.status}`)
    throw new ApiError(res.status, msg, data)
  }
  return data as T
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: "POST", body: body === undefined ? undefined : JSON.stringify(body) }),
  put: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: "PUT", body: body === undefined ? undefined : JSON.stringify(body) }),
  del: <T>(path: string) => request<T>(path, { method: "DELETE" }),
}

// —— 类型定义（与后端契约对应）——

export interface Me {
  authenticated: boolean
  csrfToken?: string
  developer?: {
    developerId: string
    displayName: string
    githubLogin: string
    avatarUrl?: string | null
    email?: string | null
    isAdmin: boolean
    isBanned: boolean
    githubConnected: boolean
    preferredLanguage: "zh" | "en"
  }
}

export interface SiteInfo {
  siteName: string
  siteNameEn?: string
  announcement: string
  publicCatalogEnabled: boolean
  officialDeveloperId: string
}

export interface CatalogItem {
  id: string
  name: string
  version: string
  developerId: string
  developerName: string
  permissions: string[]
  description: string
  descriptionEn?: string
  updatesHalted: boolean
  official: boolean
}

export interface Paged<T> {
  items: T[]
  total: number
  page: number
  pageSize: number
}

export interface PermissionInfo {
  name: string
  risk: "low" | "medium" | "high" | "unknown"
  description: string
}

export interface PluginDetail {
  id: string
  name: string
  version: string
  developerId: string
  developerName: string
  official: boolean
  status: "published" | "stopped" | "updates_halted" | "paused"
  description: string
  descriptionEn?: string
  permissions: { name: string; risk: string }[]
  repositoryUrl: string
  homepageUrl?: string | null
  docsUrl?: string | null
  updatesHaltedNotice?: string | null
  stoppedNotice?: string | null
  pausedNotice?: string | null
  updatedAt: string
}

export interface DevStats {
  total: number
  published: number
  pending: number
  draft: number
  rejected: number
  stopped: number
  paused: number
  unlisted: number
  cancelled: number
  updatesHalted: number
  pendingSubmissions: number
}

export interface AdminStats {
  pendingQueue: number
  plugins: {
    total: number
    published: number
    pending: number
    draft: number
    rejected: number
    stopped: number
    paused: number
    unlisted: number
    cancelled: number
  }
  developers: { total: number; banned: number }
  auditTotal: number
  mailFailed: number
  hostApi: {
    catalogEnabled: boolean
    sources: number
    publishedPlugins: number
  }
}

export interface DevPlugin {
  pluginId: string
  status: string
  version: string
  pendingVersion?: string | null
  stopReason?: string | null
  updatedAt: string
}

export interface DevPluginDetail {
  pluginId: string
  repositoryUrl: string
  homepageUrl?: string | null
  docsUrl?: string | null
  status: string
  displayStatus: string
  currentVersion?: string | null
  updatesHalted: boolean
  updatesHaltedReason?: string | null
  stopReason?: string | null
  submissions: {
    id: number
    version: string
    status: string
    isNewVersion: boolean
    description: string
    permissions: string[]
    downloadUrl: string
    sha256: string
    minHostVersion: string
    contractVersion: number
    reviewReason?: string | null
    ai?: { riskFlags: string[]; state: string } | null
    createdAt: string
    reviewedAt?: string | null
    isCurrent: boolean
  }[]
  timeline: {
    eventId: string
    actor: string
    action: string
    fromStatus?: string | null
    toStatus?: string | null
    reason?: string | null
    createdAt: string
  }[]
  canEdit: boolean
  canCancel: boolean
  canSubmit: boolean
  canPause: boolean
  canResumeSelf: boolean
  canUnlist: boolean
  canSubmitNewVersion: boolean
}

export interface RepoItem {
  fullName: string
  url: string
  private: boolean
  isAdmin: boolean
  description?: string | null
  updatedAt?: string | null
}

export interface RepoPage {
  items: RepoItem[]
  page: number
  hasNext: boolean
}

export interface RepoValidation {
  owner: string
  repo: string
  private: boolean
  hasAdmin: boolean
  hasReleaseZip: boolean
  ok: boolean
  errors: string[]
  /** 与 errors 一一对应的稳定错误码（前端按语言本地化） */
  errorCodes?: string[]
  defaultBranch?: string
}

export interface ManifestPreview {
  repositoryUrl: string
  defaultBranch: string
  manifest: {
    id?: string
    name?: string
    version?: string
    description?: string
    description_en?: string
    minimumHostVersion?: string | null
    contractVersion?: number
    permissions?: string[]
  }
  releases: {
    tag: string
    prerelease: boolean
    publishedAt?: string | null
    zipAssets: { name: string; size: number }[]
  }[]
}

export interface QueueItem {
  submissionId: number
  pluginId: string
  developerId: string
  developerName: string
  version: string
  name: string
  permissions: string[]
  repositoryUrl: string
  downloadUrl: string
  isNewVersion: boolean
  aiReport?: {
    riskFlags?: string[]
    recommendation?: string
    reasons?: string
  } | null
  aiState: string
  createdAt: string
  isSelf: boolean
}

export interface AuditItem {
  eventId: string
  actor: string
  actorRole: string
  action: string
  pluginId?: string | null
  submissionId?: number | null
  fromStatus?: string | null
  toStatus?: string | null
  reason?: string | null
  createdAt: string
}

export interface QueueDetail extends QueueItem {
  pluginStatus: string
  updatesHalted: boolean
  developerBanned: boolean
  description: string
  manifest: Record<string, unknown>
  sha256: string
  minHostVersion: string
  contractVersion: number
  status: string
  reviewReason?: string | null
}

export interface SmtpConfig {
  host: string
  port: string
  username: string
  hasPassword: boolean
  encryption: string
  fromAddress: string
  fromName: string
}

export interface SmtpLogItem {
  eventId: string
  to: string
  template: string
  status: "queued" | "sent" | "failed"
  subject: string
  pluginId?: string | null
  attempts: number
  error?: string | null
  createdAt: string
}

export interface AiConfig {
  enabled: boolean
  baseUrl: string
  model: string
  hasApiKey: boolean
  timeoutSec: string
}

export interface SiteConfig {
  siteName: string
  siteNameEn: string
  announcement: string
  pageSize: string
  publicCatalogEnabled: boolean
  catalogBaseUrl: string
}

export interface DevListItem {
  developerId: string
  displayName: string
  githubLogin: string
  isAdmin: boolean
  isBanned: boolean
  isSystem: boolean
  pluginCount: number
  createdAt: string
}

export interface VisitPoint {
  day: string
  pv: number
  uv: number
}

/** 宿主权限表条目（管理员可维护） */
export interface PermissionTag {
  name: string
  risk: "low" | "medium" | "high"
  description: string
  /** 被当前发布版本或待审申请声明的插件数量（>0 时禁止删除） */
  usedBy: number
}

export interface VisitSeries {
  days: number
  series: VisitPoint[]
  totalPv: number
  totalUv: number
}

/** 按当前语言本地化时间（不再硬编码 zh-CN） */
export function fmtTime(iso?: string | null): string {
  if (!iso) return "—"
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return "—"
  const locale = getLang() === "en" ? "en-US" : "zh-CN"
  return d.toLocaleString(locale, { hour12: false })
}

/** 相对等待时长（用于审核队列的「已等待」列） */
export function fmtWait(fromIso: string): string {
  const ms = Date.now() - new Date(fromIso).getTime()
  if (ms < 0) return "—"
  const mins = Math.floor(ms / 60000)
  if (mins < 1) return "<1m"
  if (mins < 60) return getLang() === "en" ? `${mins}m` : `${mins} 分钟`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return getLang() === "en" ? `${hours}h` : `${hours} 小时`
  const days = Math.floor(hours / 24)
  return getLang() === "en" ? `${days}d` : `${days} 天`
}
