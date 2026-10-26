import { useCallback, useEffect, useState } from "react"
import { Button } from "@workspace/ui/components/button"
import { Dialog } from "@workspace/ui/components/dialog"
import { Input } from "@workspace/ui/components/input"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@workspace/ui/components/table"
import { api, fmtTime, type AuditItem, type Paged } from "@/lib/api"
import { useRequireAdmin } from "@/lib/auth"
import { useToast } from "@/components/toast"
import { Pagination } from "@/components/pagination"
import { TableSkeleton } from "@/components/table-skeleton"
import { EmptyState } from "@/components/page-states"
import { PageContainer, PageHeader } from "@/components/page-header"
import { t } from "@/i18n"

/** 事件代码 → 本地化操作名；未知代码回退原始代码（技术标识保留原样） */
function actionName(code: string): string {
  return (t.auditEvents as Record<string, string>)[code] ?? code
}

function roleName(role: string): string {
  if (role === "admin") return t.misc.admin
  if (role === "system") return t.misc.system
  return t.misc.developerRole
}

function statusName(s: string | null | undefined): string {
  return s ? ((t.status as Record<string, string>)[s] ?? s) : "—"
}

export function AdminAudit() {
  const { ready, isAdmin } = useRequireAdmin()
  const { toast } = useToast()
  const [data, setData] = useState<Paged<AuditItem> | null>(null)
  const [plugin, setPlugin] = useState("")
  const [actor, setActor] = useState("")
  const [action, setAction] = useState("")
  // 时间区间筛选：默认今天（本地时区）
  const today = (() => {
    const d = new Date()
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`
  })()
  const [from, setFrom] = useState(today)
  const [to, setTo] = useState(today)
  const [page, setPage] = useState(1)
  const [detail, setDetail] = useState<AuditItem | null>(null)

  const load = useCallback(() => {
    // 本地日期 → UTC 时刻（from=当天 00:00、to=次日 00:00），"今天"边界跟随用户时区
    const toInstant = (date: string, endOfDay: boolean): string => {
      if (!date) return ""
      const d = new Date(`${date}T00:00:00`)
      if (endOfDay) d.setDate(d.getDate() + 1)
      return Number.isNaN(d.getTime()) ? "" : d.toISOString()
    }
    const params = new URLSearchParams({ page: String(page) })
    if (plugin) params.set("plugin", plugin)
    if (actor) params.set("actor", actor)
    if (action) params.set("action", action)
    if (from) params.set("from", toInstant(from, false))
    if (to) params.set("to", toInstant(to, true))
    api
      .get<Paged<AuditItem>>(`/api/admin/audit?${params.toString()}`)
      .then(setData)
      .catch((e: Error) => toast("error", e.message))
  }, [page, plugin, actor, action, from, to, toast])

  useEffect(() => {
    if (isAdmin) load()
  }, [isAdmin, load])

  if (!ready) return null
  if (!isAdmin) return <div className="text-destructive py-20 text-center text-sm">{t.errors.needAdmin}</div>

  return (
    <PageContainer>
      <div className="animate-in space-y-5 fade-in duration-300">
        <PageHeader title={t.admin.audit.title} description={t.admin.audit.subtitle} />

        <div className="flex flex-col gap-2 sm:flex-row">
          <Input
            type="search"
            aria-label={t.misc.filterByPlugin}
            placeholder={t.misc.filterByPlugin}
            value={plugin}
            onChange={(e) => {
              setPlugin(e.target.value)
              setPage(1)
            }}
            className="sm:max-w-52"
          />
          <Input
            type="search"
            aria-label={t.misc.filterByActor}
            placeholder={t.misc.filterByActor}
            value={actor}
            onChange={(e) => {
              setActor(e.target.value)
              setPage(1)
            }}
            className="sm:max-w-40"
          />
          <Input
            type="search"
            aria-label={t.misc.filterByAction}
            placeholder={t.misc.filterByAction}
            value={action}
            onChange={(e) => {
              setAction(e.target.value)
              setPage(1)
            }}
            className="sm:max-w-44"
          />
          <Input
            type="date"
            aria-label={t.admin.audit.dateFrom}
            title={t.admin.audit.dateFrom}
            value={from}
            max={to || undefined}
            onChange={(e) => {
              setFrom(e.target.value)
              setPage(1)
            }}
            className="text-muted-foreground sm:max-w-40"
          />
          <Input
            type="date"
            aria-label={t.admin.audit.dateTo}
            title={t.admin.audit.dateTo}
            value={to}
            min={from || undefined}
            onChange={(e) => {
              setTo(e.target.value)
              setPage(1)
            }}
            className="text-muted-foreground sm:max-w-40"
          />
          {from || to ? (
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                setFrom("")
                setTo("")
                setPage(1)
              }}
            >
              {t.admin.audit.allTime}
            </Button>
          ) : null}
          <Button variant="outline" size="sm" onClick={load}>
            {t.common.filter}
          </Button>
        </div>

        <div className="bg-card overflow-x-auto rounded-xl border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t.admin.audit.columns.time}</TableHead>
                <TableHead>{t.admin.audit.columns.actor}</TableHead>
                <TableHead>{t.admin.audit.columns.role}</TableHead>
                <TableHead>{t.admin.audit.columns.action}</TableHead>
                <TableHead>{t.admin.audit.columns.object}</TableHead>
                <TableHead>{t.admin.audit.columns.result}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {!data ? (
                <TableSkeleton cols={6} rows={10} widths={[20, 14, 10, 22, 22, 20]} />
              ) : data.items.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6}>
                    <EmptyState title={t.admin.audit.empty} description={t.admin.audit.emptyHint} />
                  </TableCell>
                </TableRow>
              ) : (
                data.items.map((a, i) => (
                  <TableRow key={i} className="cursor-pointer" onClick={() => setDetail(a)}>
                    <TableCell className="text-muted-foreground text-xs whitespace-nowrap">{fmtTime(a.createdAt)}</TableCell>
                    <TableCell className="font-mono text-xs">{a.actor}</TableCell>
                    <TableCell className="text-xs">{roleName(a.actorRole)}</TableCell>
                    <TableCell className="text-xs whitespace-nowrap">{actionName(a.action)}</TableCell>
                    <TableCell className="max-w-44 truncate font-mono text-xs">{a.pluginId ?? "—"}</TableCell>
                    <TableCell className="text-xs whitespace-nowrap">
                      {a.fromStatus && a.toStatus ? (
                        <>
                          {statusName(a.fromStatus)} <span className="text-muted-foreground">→</span> {statusName(a.toStatus)}
                        </>
                      ) : (
                        "—"
                      )}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>

        {data ? <Pagination page={data.page} pageSize={data.pageSize} total={data.total} onPage={setPage} /> : null}

        {/* 日志详情：原始事件代码 / 状态前后 / 原因 / 关联对象 */}
        <Dialog open={!!detail} onClose={() => setDetail(null)} title={t.admin.audit.detailTitle}>
          {detail ? (
            <dl className="grid grid-cols-[auto_1fr] gap-x-5 gap-y-2.5 text-sm">
              <dt className="text-muted-foreground">{t.admin.audit.detailEvent}</dt>
              <dd className="font-mono text-xs">{detail.action}</dd>
              <dt className="text-muted-foreground">事件 ID</dt>
              <dd className="font-mono text-xs break-all">{detail.eventId}</dd>
              <dt className="text-muted-foreground">{t.admin.audit.columns.action}</dt>
              <dd>{actionName(detail.action)}</dd>
              <dt className="text-muted-foreground">{t.admin.audit.columns.actor}</dt>
              <dd>
                <span className="font-mono text-xs">{detail.actor}</span>
                <span className="text-muted-foreground ml-2 text-xs">{roleName(detail.actorRole)}</span>
              </dd>
              <dt className="text-muted-foreground">{t.admin.audit.detailFrom}</dt>
              <dd>{statusName(detail.fromStatus)}</dd>
              <dt className="text-muted-foreground">{t.admin.audit.detailTo}</dt>
              <dd>{statusName(detail.toStatus)}</dd>
              <dt className="text-muted-foreground">{t.admin.audit.detailPlugin}</dt>
              <dd className="font-mono text-xs">{detail.pluginId ?? "—"}</dd>
              {detail.submissionId ? (
                <>
                  <dt className="text-muted-foreground">{t.admin.audit.detailSubmission}</dt>
                  <dd className="font-mono text-xs">#{detail.submissionId}</dd>
                </>
              ) : null}
              <dt className="text-muted-foreground">{t.admin.audit.columns.time}</dt>
              <dd className="text-xs">{fmtTime(detail.createdAt)}</dd>
              <dt className="text-muted-foreground">{t.admin.audit.detailReason}</dt>
              <dd className="text-xs leading-relaxed whitespace-pre-wrap">{detail.reason ?? t.admin.audit.noReason}</dd>
            </dl>
          ) : null}
        </Dialog>
      </div>
    </PageContainer>
  )
}
