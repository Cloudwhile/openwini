import { lazy, Suspense, useEffect, useState } from "react"
import { Link, useNavigate } from "react-router-dom"
import { BadgeCheck, CheckCircle2, Clock, Eye, FileClock, Users, Wrench } from "lucide-react"
import { Badge } from "@workspace/ui/components/badge"
import { StatCard } from "@/components/stat-card"
import { PageContainer, PageHeader } from "@/components/page-header"
import { api, fmtTime, type AdminStats, type AuditItem, type Paged, type VisitSeries } from "@/lib/api"
import { useRequireAdmin } from "@/lib/auth"
import { t } from "@/i18n"

// ECharts 体积较大：懒加载为独立 chunk，仅在进入概览页时拉取
const VisitChart = lazy(() => import("@/components/visit-chart").then((m) => ({ default: m.VisitChart })))

/** 宿主 App 接口可用性：真实探测 GET /api/sources（200 即可达），停用态来自配置 */
type HostState = "ok" | "disabled" | "down" | null

function useHostApiStatus(enabled: boolean | undefined): HostState {
  const [reachable, setReachable] = useState<boolean | null>(null)
  useEffect(() => {
    if (!enabled) return
    let alive = true
    fetch("/api/sources")
      .then((r) => {
        if (alive) setReachable(r.ok)
      })
      .catch(() => {
        if (alive) setReachable(false)
      })
    return () => {
      alive = false
    }
  }, [enabled])
  if (!enabled) return null
  if (reachable === null) return null
  return reachable ? "ok" : "down"
}

/** 管理后台首页：真实聚合统计 + 访问趋势 + 最近审计 */
export function AdminDashboard() {
  const { ready, isAdmin } = useRequireAdmin()
  const navigate = useNavigate()
  const [stats, setStats] = useState<AdminStats | null>(null)
  const [audit, setAudit] = useState<Paged<AuditItem> | null>(null)
  const [visits, setVisits] = useState<VisitSeries | null>(null)

  useEffect(() => {
    if (ready && !isAdmin) navigate("/")
  }, [ready, isAdmin, navigate])

  useEffect(() => {
    if (!isAdmin) return
    api.get<AdminStats>("/api/admin/stats").then(setStats).catch(() => {})
    api.get<Paged<AuditItem>>("/api/admin/audit?page=1").then(setAudit).catch(() => {})
    api.get<VisitSeries>("/api/admin/visits?days=14").then(setVisits).catch(() => {})
  }, [isAdmin])

  // 宿主接口可达性探测（hook 必须无条件调用；未拿到 stats 前不探测）
  const hostReachable = useHostApiStatus(stats?.hostApi?.catalogEnabled)

  if (!ready) return null
  if (!isAdmin) return <div className="text-destructive py-20 text-center text-sm">{t.errors.needAdmin}</div>

  const hostState: HostState = stats
    ? stats.hostApi.catalogEnabled
      ? hostReachable
      : "disabled"
    : null

  return (
    <PageContainer>
      <div className="animate-in space-y-6 fade-in duration-300">
        <PageHeader title={t.misc.overview} description={t.admin.queue.subtitle} />

        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
          <button className="text-left" onClick={() => navigate("/admin/review")}>
            <StatCard
              icon={<Clock className="size-5" aria-hidden />}
              value={stats?.pendingQueue}
              label={t.misc.pendingReviews}
              tone="warning"
              className="hover:-translate-y-0.5"
            />
          </button>
          <StatCard icon={<Wrench className="size-5" aria-hidden />} value={stats?.plugins.total} label={t.admin.plugins.title} />
          <StatCard icon={<Users className="size-5" aria-hidden />} value={stats?.developers.total} label={t.admin.developers.title} />
          <StatCard
            icon={<FileClock className="size-5" aria-hidden />}
            value={stats?.auditTotal}
            label={t.admin.audit.title}
            tone="success"
          />
        </div>

        {/* 宿主 App 接口：可达性为真实探测，聚合来自 stats */}
        <section className="bg-card rounded-xl border">
          <div className="flex flex-wrap items-center justify-between gap-2 border-b px-5 py-3">
            <h2 className="flex items-center gap-2 text-sm font-semibold">
              <BadgeCheck className="text-muted-foreground size-4" aria-hidden />
              {t.stats.hostApiTitle}
            </h2>
            {hostState ? (
              <Badge variant={hostState === "ok" ? "success" : hostState === "disabled" ? "warning" : "destructive"}>
                {hostState === "ok" ? t.stats.hostApiOk : hostState === "disabled" ? t.stats.hostApiDisabled : t.stats.hostApiDown}
              </Badge>
            ) : (
              <span className="text-muted-foreground text-xs">{t.common.loading}</span>
            )}
          </div>
          <div className="space-y-3 px-5 py-4">
            <p className="text-muted-foreground text-xs leading-relaxed">{t.stats.hostApiDesc}</p>
            <div className="flex flex-wrap gap-x-10 gap-y-3">
              <div>
                <div className="text-muted-foreground text-xs">{t.stats.sources}</div>
                <div className="font-mono text-xl font-semibold">{stats?.hostApi.sources ?? "—"}</div>
              </div>
              <div>
                <div className="text-muted-foreground text-xs">{t.stats.published}</div>
                <div className="font-mono text-xl font-semibold">{stats?.hostApi.publishedPlugins ?? "—"}</div>
              </div>
            </div>
            <div className="text-muted-foreground space-y-1 font-mono text-xs">
              <p>GET /api/sources</p>
              <p>{"GET /api/sources/{developerId}/plugins"}</p>
            </div>
          </div>
        </section>

        {/* 访问趋势（PV/UV，真实采集） */}
        <section className="bg-card rounded-xl border">
          <div className="flex flex-wrap items-center justify-between gap-2 border-b px-5 py-3">
            <h2 className="flex items-center gap-2 text-sm font-semibold">
              <Eye className="text-muted-foreground size-4" aria-hidden />
              {t.stats.siteTitle}
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

        <section className="bg-card rounded-xl border">
          <div className="flex items-center justify-between border-b px-5 py-3">
            <h2 className="text-sm font-semibold">{t.admin.audit.title}</h2>
            <Link to="/admin/audit" className="text-primary text-xs hover:underline">
              {t.misc.viewAll}
            </Link>
          </div>
          <div className="px-3 py-2">
            {!audit ? (
              <div className="text-muted-foreground py-8 px-2 text-center text-sm">{t.common.loading}</div>
            ) : audit.items.length === 0 ? (
              <div className="text-muted-foreground py-8 px-2 text-center text-sm">{t.common.empty}</div>
            ) : (
              <div className="divide-y">
                {audit.items.slice(0, 6).map((a, i) => (
                  <div key={i} className="flex items-center gap-3 px-2 py-2.5 text-xs">
                    <span className="text-muted-foreground w-36 shrink-0 whitespace-nowrap">{fmtTime(a.createdAt)}</span>
                    <span className="w-28 shrink-0 truncate font-mono">{a.actor}</span>
                    <span className="w-40 shrink-0 truncate">
                      {(t.auditEvents as Record<string, string>)[a.action] ?? a.action}
                    </span>
                    <span className="min-w-0 flex-1 truncate">
                      {a.pluginId ? <span className="font-mono">{a.pluginId}</span> : null}
                      {a.fromStatus && a.toStatus ? (
                        <span className="text-muted-foreground">
                          {" "}
                          (
                          {(t.status as Record<string, string>)[a.fromStatus] ?? a.fromStatus} →{" "}
                          {(t.status as Record<string, string>)[a.toStatus] ?? a.toStatus})
                        </span>
                      ) : null}
                    </span>
                    {a.fromStatus === "pending" && a.toStatus === "published" ? (
                      <CheckCircle2 className="text-success size-3.5 shrink-0" aria-hidden />
                    ) : null}
                  </div>
                ))}
              </div>
            )}
          </div>
        </section>
      </div>
    </PageContainer>
  )
}
