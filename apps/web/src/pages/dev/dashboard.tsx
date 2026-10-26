import { lazy, Suspense, useEffect, useState } from "react"
import { Link, useNavigate } from "react-router-dom"
import { AlertTriangle, CheckCircle2, Clock, Eye, FileX2, Plus, Wrench } from "lucide-react"
import { Button } from "@workspace/ui/components/button"
import { StatCard } from "@/components/stat-card"
import { StatusBadge } from "@/components/status-badge"
import { PageContainer, PageHeader } from "@/components/page-header"
import { ErrorState } from "@/components/page-states"
import { api, fmtTime, type DevStats, type VisitSeries } from "@/lib/api"
import { useRequireAuth } from "@/lib/auth"
import { takeLoginRedirect } from "@/lib/login-redirect"
import { t } from "@/i18n"

// ECharts 体积较大：懒加载为独立 chunk，仅在进入概览页时拉取
const VisitChart = lazy(() => import("@/components/visit-chart").then((m) => ({ default: m.VisitChart })))

interface RecentList {
  items: { pluginId: string; status: string; version: string; updatedAt: string }[]
}

/** 开发者后台首页：统计概览（真实聚合）+ 访问趋势 + 待处理事项 + 最近插件 */
export function DeveloperDashboard() {
  const { me, ready, logged } = useRequireAuth()
  const navigate = useNavigate()
  const [stats, setStats] = useState<DevStats | null>(null)
  const [recent, setRecent] = useState<RecentList | null>(null)
  const [visits, setVisits] = useState<VisitSeries | null>(null)
  const [error, setError] = useState("")

  useEffect(() => {
    if (ready && !logged) navigate("/login")
  }, [ready, logged, navigate])

  // 登录前记录的目标页（会话过期跳转）：进仪表盘后续跳一次
  useEffect(() => {
    if (!ready || !logged) return
    const redirect = takeLoginRedirect()
    if (redirect) {
      navigate(redirect, { replace: true })
    }
  }, [ready, logged, navigate])

  const load = () => {
    api
      .get<DevStats>("/api/dev/stats")
      .then((s) => {
        setStats(s)
        setError("")
      })
      .catch((e: Error) => setError(e.message))
    api
      .get<RecentList>("/api/dev/plugins?page=1")
      .then(setRecent)
      .catch(() => {})
    api
      .get<VisitSeries>("/api/dev/visits?days=14")
      .then(setVisits)
      .catch(() => {})
  }

  useEffect(() => {
    if (logged) load()
  }, [logged])

  if (!ready || !logged) return null
  const banned = me?.developer?.isBanned

  // 待处理事项：只来自真实数据
  const todos: { text: string; to: string }[] = []
  if (stats) {
    if (stats.updatesHalted > 0) todos.push({ text: t.devStats.todoHalted(stats.updatesHalted), to: "/me/plugins" })
    if (stats.rejected > 0) todos.push({ text: t.devStats.todoRejected(stats.rejected), to: "/me/plugins" })
    if (stats.pending > 0 || stats.pendingSubmissions > 0)
      todos.push({ text: t.devStats.todoPending(Math.max(stats.pending, stats.pendingSubmissions)), to: "/me/plugins" })
  }

  return (
    <PageContainer>
      <div className="animate-in space-y-6 fade-in duration-300">
        <PageHeader
          title={t.devStats.title}
          description={t.devStats.subtitle}
          actions={
            <Button onClick={() => navigate("/me/plugins/new")}>
              <Plus className="size-4" aria-hidden />
              {t.me.newPlugin}
            </Button>
          }
        />

        {banned ? (
          <div role="alert" className="border-destructive/40 bg-destructive/5 rounded-lg border p-3 text-sm text-destructive">
            {t.me.bannedWarning}
          </div>
        ) : null}

        {error ? <ErrorState message={error} onRetry={load} /> : null}

        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
          <StatCard icon={<Wrench className="size-5" aria-hidden />} value={stats?.total} label={t.devStats.total} />
          <StatCard
            icon={<CheckCircle2 className="size-5" aria-hidden />}
            value={stats?.published}
            label={t.devStats.published}
            tone="success"
          />
          <StatCard
            icon={<Clock className="size-5" aria-hidden />}
            value={stats ? stats.pending + stats.pendingSubmissions : undefined}
            label={t.devStats.pending}
            tone="warning"
          />
          <StatCard
            icon={<FileX2 className="size-5" aria-hidden />}
            value={stats?.rejected}
            label={t.devStats.rejected}
            tone="destructive"
          />
        </div>

        {/* 访问趋势（PV/UV，真实采集，仅统计名下插件详情页） */}
        <section className="bg-card rounded-xl border">
          <div className="flex flex-wrap items-center justify-between gap-2 border-b px-5 py-3">
            <h2 className="text-muted-foreground flex items-center gap-2 text-sm font-semibold">
              <Eye className="size-4" aria-hidden />
              {t.stats.devTitle}
            </h2>
            {visits ? (
              <div className="text-muted-foreground flex items-center gap-3 text-xs">
                <span>
                  {t.stats.pv} <strong className="text-foreground font-mono">{visits.totalPv}</strong>
                </span>
                <span>
                  {t.stats.uv} <strong className="text-foreground font-mono">{visits.totalUv}</strong>
                </span>
              </div>
            ) : null}
          </div>
          <div className="px-5 py-4">
            {visits ? (
              <Suspense fallback={<div className="text-muted-foreground py-10 text-center text-sm">{t.common.loading}</div>}>
                <VisitChart series={visits.series} days={visits.days} />
              </Suspense>
            ) : (
              <div className="text-muted-foreground py-10 text-center text-sm">{t.common.loading}</div>
            )}
          </div>
        </section>

        {/* 待处理事项 */}
        <section className="bg-card rounded-xl border">
          <div className="border-b px-5 py-3">
            <h2 className="text-sm font-semibold">{t.devStats.todo}</h2>
          </div>
          <div className="px-5 py-3">
            {stats === null ? (
              <p className="text-muted-foreground py-3 text-sm">{t.common.loading}</p>
            ) : todos.length === 0 ? (
              <p className="text-muted-foreground py-3 text-sm">{t.devStats.todoEmpty}</p>
            ) : (
              <ul className="divide-y">
                {todos.map((todo) => (
                  <li key={todo.text}>
                    <Link
                      to={todo.to}
                      className="text-muted-foreground hover:text-foreground flex items-center gap-2.5 py-2.5 text-sm transition-colors"
                    >
                      <AlertTriangle className="text-warning size-4 shrink-0" aria-hidden />
                      <span className="min-w-0 flex-1">{todo.text}</span>
                      <span aria-hidden>→</span>
                    </Link>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </section>

        {/* 最近插件 */}
        <section className="bg-card rounded-xl border">
          <div className="flex items-center justify-between border-b px-5 py-3">
            <h2 className="text-sm font-semibold">{t.devStats.recent}</h2>
            <Link to="/me/plugins" className="text-primary text-xs hover:underline">
              {t.devStats.viewAll}
            </Link>
          </div>
          <div className="px-3 py-2">
            {!recent ? (
              <p className="text-muted-foreground py-3 px-2 text-sm">{t.common.loading}</p>
            ) : recent.items.length === 0 ? (
              <p className="text-muted-foreground py-3 px-2 text-sm">{t.me.empty}</p>
            ) : (
              <div className="divide-y">
                {recent.items.slice(0, 5).map((p) => (
                  <Link
                    key={p.pluginId}
                    to={`/me/plugins/${encodeURIComponent(p.pluginId)}`}
                    className="hover:bg-muted/50 flex items-center gap-3 rounded-lg px-2 py-2.5 transition-colors"
                  >
                    <span className="min-w-0 flex-1 truncate font-mono text-xs">{p.pluginId}</span>
                    <StatusBadge status={p.status} />
                    <span className="text-muted-foreground w-16 text-xs">{p.version || "—"}</span>
                    <span className="text-muted-foreground hidden w-36 text-xs sm:inline">{fmtTime(p.updatedAt)}</span>
                  </Link>
                ))}
              </div>
            )}
          </div>
        </section>
      </div>
    </PageContainer>
  )
}
