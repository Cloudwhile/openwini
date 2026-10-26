import { useCallback, useEffect, useState } from "react"
import { Link } from "react-router-dom"
import { ClipboardList, ExternalLink, Loader2, ShieldAlert } from "lucide-react"
import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"
import { Dialog } from "@workspace/ui/components/dialog"
import { Input } from "@workspace/ui/components/input"
import { Label } from "@workspace/ui/components/label"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@workspace/ui/components/table"
import { Textarea } from "@workspace/ui/components/textarea"
import { api, fmtTime, fmtWait, type Paged, type PermissionInfo, type QueueDetail, type QueueItem } from "@/lib/api"
import { useRequireAdmin } from "@/lib/auth"
import { useToast } from "@/components/toast"
import { Pagination } from "@/components/pagination"
import { PermissionBadges } from "@/components/permission-badge"
import { EmptyState, ErrorState, LoadingState } from "@/components/page-states"
import { SearchableSelect } from "@/components/searchable-select"
import { PageContainer, PageHeader } from "@/components/page-header"
import { t } from "@/i18n"

const REJECT_TEMPLATES = [
  t.admin.queue.rejectTemplates.manifest,
  t.admin.queue.rejectTemplates.permission,
  t.admin.queue.rejectTemplates.description,
  t.admin.queue.rejectTemplates.repo,
]

function AiBadge({ state, report }: { state: string; report?: QueueItem["aiReport"] }) {
  const rec = report?.recommendation
  if (state === "failed") return <Badge variant="warning">{t.admin.queue.aiState.failed}</Badge>
  if (state === "skipped" || !report) return <Badge variant="secondary">{t.admin.queue.aiState.skipped}</Badge>
  if (rec === "approve") return <Badge variant="success">{t.admin.queue.aiRecommend.approve}</Badge>
  if (rec === "reject") return <Badge variant="destructive">{t.admin.queue.aiRecommend.reject}</Badge>
  return <Badge variant="warning">{t.admin.queue.aiRecommend.manual}</Badge>
}

/** 权限风险概览：高危数量 > 0 时给出醒目提示（审核风险一眼可见） */
function RiskSummary({ permissions, known }: { permissions: string[]; known: Record<string, PermissionInfo> | null }) {
  const high = permissions.filter((p) => known?.[p]?.risk === "high").length
  if (high > 0) {
    return (
      <span className="text-destructive inline-flex items-center gap-1 text-xs font-medium">
        <ShieldAlert className="size-3.5" aria-hidden />
        {t.admin.queue.highRiskPerms(high)}
      </span>
    )
  }
  return <PermissionBadges permissions={permissions.slice(0, 3)} known={known} showRisk={false} />
}

export function ReviewQueue() {
  const { ready, isAdmin } = useRequireAdmin()
  const { toast } = useToast()
  const [data, setData] = useState<Paged<QueueItem> | null>(null)
  const [developer, setDeveloper] = useState("")
  const [permission, setPermission] = useState("")
  const [page, setPage] = useState(1)
  const [known, setKnown] = useState<Record<string, PermissionInfo> | null>(null)
  const [detail, setDetail] = useState<QueueDetail | null>(null)
  const [detailLoading, setDetailLoading] = useState(false)
  const [rejecting, setRejecting] = useState<QueueItem | null>(null)
  const [approveTarget, setApproveTarget] = useState<QueueItem | null>(null)
  const [reason, setReason] = useState("")
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")

  const load = useCallback(() => {
    api
      .get<Paged<QueueItem>>(
        `/api/admin/queue?page=${page}&developer=${encodeURIComponent(developer)}&permission=${encodeURIComponent(permission)}`,
      )
      .then((d) => {
        setData(d)
        setError("")
      })
      .catch((e: Error) => setError(e.message))
  }, [page, developer, permission])

  useEffect(() => {
    if (isAdmin) load()
  }, [isAdmin, load])

  useEffect(() => {
    api
      .get<{ permissions: PermissionInfo[] }>("/api/public/permissions")
      .then((r) => {
        const m: Record<string, PermissionInfo> = {}
        for (const p of r.permissions) m[p.name] = p
        setKnown(m)
      })
      .catch(() => {})
  }, [])

  const openDetail = async (item: QueueItem) => {
    setDetailLoading(true)
    try {
      const d = await api.get<QueueDetail>(`/api/admin/queue/${item.submissionId}`)
      setDetail(d)
    } catch (e) {
      toast("error", (e as Error).message)
    } finally {
      setDetailLoading(false)
    }
  }

  if (!ready) return null
  if (!isAdmin) return <div className="text-destructive py-20 text-center text-sm">{t.errors.needAdmin}</div>

  const approve = async (item: QueueItem) => {
    setBusy(true)
    try {
      await api.post(`/api/admin/queue/${item.submissionId}/approve`, {})
      toast("success", t.misc.approvedPublished)
      setDetail(null)
      setApproveTarget(null)
      load()
    } catch (e) {
      toast("error", (e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const reject = async () => {
    if (!rejecting || !reason.trim()) return
    setBusy(true)
    try {
      await api.post(`/api/admin/queue/${rejecting.submissionId}/reject`, { reason: reason.trim() })
      toast("success", t.misc.rejectedNotified)
      setRejecting(null)
      setReason("")
      setDetail(null)
      load()
    } catch (e) {
      toast("error", (e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <PageContainer>
      <div className="animate-in space-y-5 fade-in duration-300">
        <PageHeader
          title={t.admin.queue.title}
          description={t.admin.queue.subtitle}
          actions={
            data ? (
              <Badge variant={data.total > 0 ? "warning" : "secondary"} className="font-mono text-xs">
                {t.common.total(data.total)}
              </Badge>
            ) : null
          }
        />

        <div className="flex flex-col gap-2 sm:flex-row">
          <Input
            type="search"
            aria-label={t.admin.queue.developerFilter}
            placeholder={t.admin.queue.developerFilter}
            value={developer}
            onChange={(e) => {
              setDeveloper(e.target.value)
              setPage(1)
            }}
            className="sm:max-w-56"
          />
          <SearchableSelect
            searchable={false}
            ariaLabel={t.admin.queue.permissionFilter}
            value={permission}
            onChange={(v) => {
              setPermission(v)
              setPage(1)
            }}
            className="sm:max-w-44"
            options={[
              { value: "", label: t.common.all },
              ...(known ? Object.values(known).map((p) => ({ value: p.name, label: p.name })) : []),
            ]}
          />
        </div>

        {error ? <ErrorState message={error} onRetry={load} /> : null}

        <div className="bg-card overflow-x-auto rounded-xl border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t.admin.queue.columns.plugin}</TableHead>
                <TableHead>{t.admin.queue.columns.developer}</TableHead>
                <TableHead>{t.admin.queue.columns.version}</TableHead>
                <TableHead>{t.admin.queue.columns.permissions}</TableHead>
                <TableHead>{t.admin.queue.columns.type}</TableHead>
                <TableHead>{t.admin.queue.columns.ai}</TableHead>
                <TableHead>{t.admin.queue.columns.submitted}</TableHead>
                <TableHead>{t.admin.queue.columns.waited}</TableHead>
                <TableHead className="text-right">{t.admin.queue.columns.actions}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {!data ? (
                <TableRow>
                  <TableCell colSpan={9}>
                    <LoadingState className="py-10" />
                  </TableCell>
                </TableRow>
              ) : data.items.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={9}>
                    <EmptyState
                      icon={<ClipboardList className="size-5" />}
                      title={t.admin.queue.empty}
                      description={t.admin.queue.emptyHint}
                    />
                  </TableCell>
                </TableRow>
              ) : (
                data.items.map((item) => (
                  <TableRow key={item.submissionId}>
                    <TableCell>
                      <button className="font-mono text-xs hover:underline" onClick={() => void openDetail(item)}>
                        {item.pluginId}
                      </button>
                      {item.isSelf ? (
                        <Badge variant="outline" className="ml-1.5">
                          {t.misc.self}
                        </Badge>
                      ) : null}
                    </TableCell>
                    <TableCell>
                      <Link to={`/developers/${item.developerId}`} className="font-mono text-xs hover:underline">
                        {item.developerId}
                      </Link>
                    </TableCell>
                    <TableCell className="font-mono text-xs">{item.version}</TableCell>
                    <TableCell className="max-w-56">
                      <RiskSummary permissions={item.permissions} known={known} />
                    </TableCell>
                    <TableCell className="text-xs">
                      {item.isNewVersion ? t.admin.queue.versionUpdate : t.admin.queue.firstPublish}
                    </TableCell>
                    <TableCell>
                      <AiBadge state={item.aiState} report={item.aiReport} />
                    </TableCell>
                    <TableCell className="text-muted-foreground text-xs">{fmtTime(item.createdAt)}</TableCell>
                    <TableCell className="text-muted-foreground text-xs">{fmtWait(item.createdAt)}</TableCell>
                    <TableCell>
                      <div className="flex items-center justify-end gap-1">
                        <Button size="xs" disabled={busy} onClick={() => setApproveTarget(item)}>
                          {t.admin.queue.approve}
                        </Button>
                        <Button
                          size="xs"
                          variant="destructive"
                          onClick={() => {
                            setRejecting(item)
                            setReason("")
                          }}
                        >
                          {t.admin.queue.reject}
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>

        {data ? <Pagination page={data.page} pageSize={data.pageSize} total={data.total} onPage={setPage} /> : null}

        {/* 审核详情：清单/权限 + AI 预审 + 版本产物 */}
        <Dialog open={!!detail || detailLoading} onClose={() => setDetail(null)} title={detail ? `${detail.name} ${detail.version}` : t.admin.queue.detailTitle}>
          {detailLoading || !detail ? (
            <LoadingState />
          ) : (
            <div className="space-y-4 text-sm">
              <div className="flex flex-wrap items-center gap-2 text-xs">
                <span className="font-mono">{detail.pluginId}</span>
                <span className="text-muted-foreground">·</span>
                <span className="text-muted-foreground">{t.admin.queue.columns.developer}</span>
                <Link className="font-mono hover:underline" to={`/developers/${detail.developerId}`}>
                  {detail.developerId}
                </Link>
                <span className="text-muted-foreground">·</span>
                <a href={detail.repositoryUrl} target="_blank" rel="noreferrer noopener" className="inline-flex items-center gap-0.5 hover:underline">
                  {detail.repositoryUrl.replace("https://github.com/", "")}
                  <ExternalLink className="size-3" aria-hidden />
                </a>
                {detail.developerBanned ? <Badge variant="destructive">{t.misc.banned}</Badge> : null}
                {detail.isSelf ? <Badge variant="outline">{t.misc.self}</Badge> : null}
              </div>

              {detail.description ? (
                <p className="text-muted-foreground rounded-lg border p-3 text-xs leading-relaxed">{detail.description}</p>
              ) : null}

              <div>
                <div className="mb-1.5 text-xs font-medium">{t.detail.permissions}</div>
                <PermissionBadges permissions={detail.permissions} known={known} />
              </div>

              {detail.aiReport || detail.aiState !== "skipped" ? (
                <div className="rounded-lg border p-3">
                  <div className="mb-1 flex items-center gap-2 text-xs">
                    <span className="text-muted-foreground font-medium">{t.admin.queue.columns.ai}</span>
                    <AiBadge state={detail.aiState} report={detail.aiReport} />
                  </div>
                  {detail.aiReport?.riskFlags?.length ? (
                    <ul className="list-disc space-y-0.5 pl-5 text-xs">
                      {detail.aiReport.riskFlags.map((f, i) => (
                        <li key={i}>{f}</li>
                      ))}
                    </ul>
                  ) : null}
                  {detail.aiReport?.reasons ? <p className="text-muted-foreground mt-1 text-xs">{detail.aiReport.reasons}</p> : null}
                  <p className="text-muted-foreground/80 mt-1.5 text-[11px]">{t.admin.queue.aiAssistNote}</p>
                </div>
              ) : null}

              {/* 版本与产物 */}
              <div className="rounded-lg border p-3">
                <div className="mb-1.5 text-xs font-medium">{t.admin.queue.tabMeta}</div>
                <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
                  <dt className="text-muted-foreground">{t.admin.queue.columns.version}</dt>
                  <dd className="font-mono">{detail.version}</dd>
                  <dt className="text-muted-foreground">SHA-256</dt>
                  <dd className="max-w-56 truncate font-mono" title={detail.sha256}>
                    {detail.sha256 || "—"}
                  </dd>
                  <dt className="text-muted-foreground">{t.submit.manifestMinHost}</dt>
                  <dd className="font-mono">{detail.minHostVersion || "—"}</dd>
                  <dt className="text-muted-foreground">{t.submit.manifestContract}</dt>
                  <dd className="font-mono">{detail.contractVersion}</dd>
                  <dt className="text-muted-foreground">{t.submit.confirm.dist}</dt>
                  <dd className="max-w-56 truncate">
                    <a href={detail.downloadUrl} target="_blank" rel="noreferrer noopener" className="hover:underline">
                      {detail.downloadUrl}
                    </a>
                  </dd>
                </dl>
              </div>

              {/* 清单 JSON */}
              <details className="rounded-lg border p-3">
                <summary className="text-muted-foreground cursor-pointer text-xs font-medium">{t.admin.queue.manifestJson}</summary>
                <pre className="bg-muted mt-2 max-h-56 overflow-auto rounded-md p-2.5 font-mono text-[11px] leading-relaxed">
                  {JSON.stringify(detail.manifest, null, 2)}
                </pre>
              </details>

              <div className="flex items-center justify-end gap-2 border-t pt-3">
                {detail.developerBanned ? (
                  <span className="text-destructive mr-auto text-xs">{t.admin.queue.devBanned}</span>
                ) : null}
                <Button variant="outline" onClick={() => setDetail(null)}>
                  {t.common.close}
                </Button>
                <Button size="sm" disabled={busy || detail.developerBanned} onClick={() => { setDetail(null); setApproveTarget(detail) }}>
                  {t.admin.queue.approve}
                </Button>
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={() => {
                    setRejecting(detail)
                    setDetail(null)
                    setReason("")
                  }}
                >
                  {t.admin.queue.reject}
                </Button>
              </div>
            </div>
          )}
        </Dialog>

        {/* 退回原因（模板 + 自定义） */}
        <Dialog
          open={!!rejecting}
          onClose={() => setRejecting(null)}
          title={t.admin.queue.rejectTitle}
          footer={
            <>
              <Button variant="outline" onClick={() => setRejecting(null)}>
                {t.common.cancel}
              </Button>
              <Button variant="destructive" disabled={busy || !reason.trim()} onClick={reject}>
                {busy ? <Loader2 className="size-4 animate-spin" aria-hidden /> : null}
                {t.admin.queue.reject}
              </Button>
            </>
          }
        >
          <div className="space-y-3">
            <div className="space-y-1.5">
              <Label>{t.admin.queue.rejectTemplates.label}</Label>
              <SearchableSelect
                searchable={false}
                clearable
                onClear={() => setReason("")}
                ariaLabel={t.admin.queue.rejectTemplates.label}
                value={null}
                onChange={(v) => setReason(v)}
                placeholder={t.misc.choose}
                options={REJECT_TEMPLATES.map((tpl) => ({ value: tpl, label: tpl }))}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="reject-reason">{t.admin.queue.customReason}</Label>
              <Textarea id="reject-reason" value={reason} rows={4} onChange={(e) => setReason(e.target.value)} />
            </div>
          </div>
        </Dialog>

        {/* 通过并发布：主题化确认弹窗（发布即对宿主可见） */}
        <Dialog
          open={!!approveTarget}
          onClose={() => setApproveTarget(null)}
          title={t.admin.queue.approveTitle}
          footer={
            <>
              <Button variant="outline" onClick={() => setApproveTarget(null)}>
                {t.common.cancel}
              </Button>
              <Button disabled={busy} onClick={() => approveTarget && approve(approveTarget)}>
                {busy ? <Loader2 className="size-4 animate-spin" aria-hidden /> : null}
                {t.admin.queue.approve}
              </Button>
            </>
          }
        >
          {approveTarget ? (
            <div className="space-y-2 text-sm">
              <div className="font-mono text-xs">
                {approveTarget.pluginId} · v{approveTarget.version}
              </div>
              <p className="text-muted-foreground leading-relaxed">{t.admin.queue.approveConfirm}</p>
            </div>
          ) : null}
        </Dialog>
      </div>
    </PageContainer>
  )
}
