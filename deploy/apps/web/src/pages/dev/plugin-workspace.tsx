import { useCallback, useEffect, useState } from "react"
import { useNavigate, useParams } from "react-router-dom"
import { ExternalLink, Info, ShieldAlert } from "lucide-react"
import { Button } from "@workspace/ui/components/button"
import { Card, CardContent, CardHeader, CardTitle } from "@workspace/ui/components/card"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@workspace/ui/components/table"
import { api, fmtTime, type DevPluginDetail } from "@/lib/api"
import { useToast } from "@/components/toast"
import { StatusBadge } from "@/components/status-badge"
import { ConfirmDialog } from "@/components/confirm-dialog"
import { ErrorState, LoadingState } from "@/components/page-states"
import { PageContainer } from "@/components/page-header"
import { t } from "@/i18n"

export function PluginWorkspace() {
  const { pluginId } = useParams<{ pluginId: string }>()
  const navigate = useNavigate()
  const { toast } = useToast()
  const [detail, setDetail] = useState<DevPluginDetail | null>(null)
  const [error, setError] = useState("")
  const [confirmCancel, setConfirmCancel] = useState(false)
  const [confirmPause, setConfirmPause] = useState(false)
  const [confirmResume, setConfirmResume] = useState(false)
  const [confirmUnlist, setConfirmUnlist] = useState(false)

  const load = useCallback(() => {
    if (!pluginId) return
    api
      .get<DevPluginDetail>(`/api/dev/plugins/${encodeURIComponent(pluginId)}`)
      .then((d) => {
        setDetail(d)
        setError("")
      })
      .catch((e: Error) => setError(e.message))
  }, [pluginId])

  useEffect(load, [load])

  if (error) {
    return (
      <PageContainer>
        <ErrorState message={error} onRetry={load} />
      </PageContainer>
    )
  }
  if (!detail)
    return (
      <PageContainer>
        <LoadingState />
      </PageContainer>
    )

  const cancel = async () => {
    try {
      await api.post(`/api/dev/plugins/${encodeURIComponent(detail.pluginId)}/cancel`)
      toast("success", t.misc.cancelled)
      load()
    } catch (e) {
      toast("error", (e as Error).message)
    }
  }

  const pause = async () => {
    try {
      await api.post(`/api/dev/plugins/${encodeURIComponent(detail.pluginId)}/pause`)
      toast("success", t.pluginActions.pause)
      load()
    } catch (e) {
      toast("error", (e as Error).message)
    }
  }
  const resumeSelf = async () => {
    try {
      await api.post(`/api/dev/plugins/${encodeURIComponent(detail.pluginId)}/resume`)
      toast("success", t.pluginActions.resumeSelf)
      load()
    } catch (e) {
      toast("error", (e as Error).message)
    }
  }
  const unlist = async () => {
    try {
      await api.post(`/api/dev/plugins/${encodeURIComponent(detail.pluginId)}/unlist`)
      toast("success", t.pluginActions.unlist)
      load()
    } catch (e) {
      toast("error", (e as Error).message)
    }
  }

  const goSubmit = (mode: "edit" | "newVersion") =>
    navigate(`/me/plugins/${encodeURIComponent(detail.pluginId)}/${mode === "edit" ? "edit" : "new-version"}`)

  return (
    <PageContainer>
      <div className="animate-in space-y-6 fade-in duration-300">
        {/* 头部：标识 + 展示状态 + 操作（由后端 can* 标志驱动，不再前端猜测） */}
        <div className="flex flex-wrap items-center gap-3">
          <h1 className="font-mono text-lg font-semibold">{detail.pluginId}</h1>
          <StatusBadge status={detail.displayStatus} />
          <div className="ml-auto flex flex-wrap gap-2">
            {detail.canSubmitNewVersion ? (
              <Button size="sm" onClick={() => goSubmit("newVersion")}>
                {t.pluginActions.newVersion}
              </Button>
            ) : null}
            {detail.canSubmit && detail.status !== "pending" ? (
              <Button size="sm" onClick={() => goSubmit("edit")}>
                {t.submit.resubmit}
              </Button>
            ) : null}
            {detail.canEdit ? (
              <Button variant="outline" size="sm" onClick={() => goSubmit("edit")}>
                {t.pluginActions.edit}
              </Button>
            ) : null}
            {detail.canPause ? (
              <Button variant="outline" size="sm" onClick={() => setConfirmPause(true)}>
                {t.pluginActions.pause}
              </Button>
            ) : null}
            {detail.canResumeSelf ? (
              <Button size="sm" onClick={() => setConfirmResume(true)}>
                {t.pluginActions.resumeSelf}
              </Button>
            ) : null}
            {detail.canUnlist ? (
              <Button variant="destructive" size="sm" onClick={() => setConfirmUnlist(true)}>
                {t.pluginActions.unlist}
              </Button>
            ) : null}
            {detail.canCancel ? (
              <Button variant="destructive" size="sm" onClick={() => setConfirmCancel(true)}>
                {t.pluginActions.cancel}
              </Button>
            ) : null}
          </div>
        </div>

        {detail.status === "unlisted" ? (
          <div role="status" className="border-warning/40 bg-warning/5 rounded-lg border p-3 text-sm leading-relaxed">
            {t.pluginActions.relistHint}
          </div>
        ) : null}

        {/* 平台冻结更新：说明原因与恢复路径（暂停/恢复不会解除冻结，需管理员操作） */}
        {detail.updatesHalted ? (
          <div role="status" className="border-warning/40 bg-warning/5 flex items-start gap-2.5 rounded-lg border p-3.5 text-sm leading-relaxed">
            <Info className="text-warning mt-0.5 size-4 shrink-0" aria-hidden />
            <div>
              <p>
                <strong className="font-medium">{t.misc.haltReasonPrefix}</strong>
                {detail.updatesHaltedReason || "—"}
              </p>
              {detail.status === "published" ? (
                <p className="text-muted-foreground mt-1 text-xs">{t.pluginActions.haltResumeHint}</p>
              ) : null}
            </div>
          </div>
        ) : null}
        {detail.status === "stopped" && detail.stopReason ? (
          <div role="alert" className="border-destructive/40 bg-destructive/5 flex items-start gap-2.5 rounded-lg border p-3.5 text-sm leading-relaxed">
            <ShieldAlert className="text-destructive mt-0.5 size-4 shrink-0" aria-hidden />
            <p>
              <strong className="font-medium">{t.misc.stopReasonPrefix}</strong>
              {detail.stopReason}
            </p>
          </div>
        ) : null}

        <div className="grid gap-4 md:grid-cols-2">
          <Card className="gap-2 py-4">
            <CardHeader>
              <CardTitle className="text-sm">{t.misc.basicInfo}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-1.5 text-sm">
              <div className="flex justify-between gap-2">
                <span className="text-muted-foreground shrink-0">{t.submit.repoLabel}</span>
                <a
                  href={detail.repositoryUrl}
                  target="_blank"
                  rel="noreferrer noopener"
                  className="flex items-center gap-1 truncate hover:underline"
                >
                  {detail.repositoryUrl.replace("https://github.com/", "")}
                  <ExternalLink className="size-3 shrink-0" aria-hidden />
                </a>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">{t.detail.version}</span>
                <span className="font-mono text-xs">{detail.currentVersion || "—"}</span>
              </div>
              {detail.homepageUrl ? (
                <div className="flex justify-between gap-2">
                  <span className="text-muted-foreground shrink-0">{t.submit.homepage}</span>
                  <a href={detail.homepageUrl} target="_blank" rel="noreferrer noopener" className="truncate hover:underline">
                    {detail.homepageUrl}
                  </a>
                </div>
              ) : null}
              {detail.docsUrl ? (
                <div className="flex justify-between gap-2">
                  <span className="text-muted-foreground shrink-0">{t.submit.docs}</span>
                  <a href={detail.docsUrl} target="_blank" rel="noreferrer noopener" className="truncate hover:underline">
                    {detail.docsUrl}
                  </a>
                </div>
              ) : null}
            </CardContent>
          </Card>

          <Card className="gap-2 py-4">
            <CardHeader>
              <CardTitle className="text-sm">{t.misc.statusNote}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-1.5 text-sm">
              {detail.status === "rejected" && !detail.stopReason ? (
                <div className="text-muted-foreground leading-relaxed">
                  {t.status.rejected} · {t.me.viewFeedback}
                </div>
              ) : null}
              {detail.status === "paused" ? <div className="text-muted-foreground">{t.status.paused}</div> : null}
              {!detail.stopReason && !detail.updatesHalted && detail.status !== "rejected" && detail.status !== "paused" ? (
                <span className="text-muted-foreground">{t.misc.noNotes}</span>
              ) : null}
            </CardContent>
          </Card>
        </div>

        <Card className="py-0">
          <CardHeader className="pt-4">
            <CardTitle className="text-sm">{t.misc.historyTitle}</CardTitle>
          </CardHeader>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t.me.columns.version}</TableHead>
                  <TableHead>{t.me.columns.status}</TableHead>
                  <TableHead>{t.admin.queue.columns.type}</TableHead>
                  <TableHead>{t.admin.queue.columns.submitted}</TableHead>
                  <TableHead>{t.admin.audit.columns.time}</TableHead>
                  <TableHead>{t.admin.audit.detailEvent}</TableHead>
                  <TableHead>{t.submit.manifestCompare}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {detail.submissions.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={6} className="text-muted-foreground py-8 text-center">
                      {t.misc.noSubmissions}
                    </TableCell>
                  </TableRow>
                ) : (
                  detail.submissions.map((s) => (
                    <TableRow key={s.id}>
                      <TableCell className="font-mono text-xs">
                        {s.version} {s.isCurrent ? <span className="text-muted-foreground">{t.misc.current}</span> : null}
                      </TableCell>
                      <TableCell>
                        <StatusBadge status={s.status} />
                      </TableCell>
                      <TableCell className="text-xs">
                        {s.isNewVersion ? t.admin.queue.versionUpdate : t.misc.firstApply}
                      </TableCell>
                      <TableCell className="text-muted-foreground text-xs">{fmtTime(s.createdAt)}</TableCell>
                      <TableCell className="text-muted-foreground text-xs">{fmtTime(s.reviewedAt)}</TableCell>
                      <TableCell className="max-w-80 text-xs whitespace-normal">
                        {s.reviewReason ? (
                          <div>
                            <span aria-hidden>📋 </span>
                            {s.reviewReason}
                          </div>
                        ) : null}
                        {s.ai?.riskFlags?.length ? (
                          <div className="text-muted-foreground mt-1">
                            <span aria-hidden>🤖 </span>
                            {s.ai.riskFlags.join("；")}
                          </div>
                        ) : null}
                        {!s.reviewReason && !s.ai?.riskFlags?.length ? "—" : null}
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </div>
        </Card>

        <Card className="py-0">
          <CardHeader className="pt-4">
            <CardTitle className="text-sm">{t.misc.timelineTitle}</CardTitle>
          </CardHeader>
          <div className="overflow-x-auto">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t.admin.audit.columns.time}</TableHead>
                  <TableHead>{t.admin.audit.columns.actor}</TableHead>
                  <TableHead>{t.admin.audit.columns.action}</TableHead>
                  <TableHead>{t.admin.audit.columns.result}</TableHead>
                  <TableHead>{t.admin.audit.columns.reason}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {detail.timeline.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={6} className="text-muted-foreground py-8 text-center">
                      {t.common.empty}
                    </TableCell>
                  </TableRow>
                ) : (
                  detail.timeline.map((row, i) => (
                    <TableRow key={i}>
                      <TableCell className="text-muted-foreground text-xs">{fmtTime(row.createdAt)}</TableCell>
                      <TableCell className="font-mono text-xs">{row.eventId}</TableCell>
                      <TableCell className="font-mono text-xs">{row.actor}</TableCell>
                      <TableCell className="text-xs">{t.auditEvents[row.action as keyof typeof t.auditEvents] ?? row.action}</TableCell>
                      <TableCell className="text-xs">
                        {row.fromStatus && row.toStatus
                          ? `${t.status[row.fromStatus as keyof typeof t.status] ?? row.fromStatus} → ${t.status[row.toStatus as keyof typeof t.status] ?? row.toStatus}`
                          : "—"}
                      </TableCell>
                      <TableCell className="max-w-80 text-xs whitespace-normal">{row.reason || "—"}</TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </div>
        </Card>

        <ConfirmDialog
          open={confirmCancel}
          onClose={() => setConfirmCancel(false)}
          onConfirm={cancel}
          title={t.pluginActions.cancelTitle}
        >
          {t.pluginActions.cancelConfirm}
        </ConfirmDialog>

        <ConfirmDialog
          open={confirmPause}
          onClose={() => setConfirmPause(false)}
          onConfirm={pause}
          title={t.pluginActions.pauseTitle}
          confirmText={t.pluginActions.pause}
        >
          {t.pluginActions.pauseConfirm}
        </ConfirmDialog>
        <ConfirmDialog
          open={confirmResume}
          onClose={() => setConfirmResume(false)}
          onConfirm={resumeSelf}
          title={t.pluginActions.resumeTitle}
          confirmText={t.pluginActions.resumeSelf}
          destructive={false}
        >
          {t.pluginActions.resumeConfirm}
        </ConfirmDialog>
        <ConfirmDialog
          open={confirmUnlist}
          onClose={() => setConfirmUnlist(false)}
          onConfirm={unlist}
          title={t.pluginActions.unlistTitle}
          confirmText={t.pluginActions.unlist}
        >
          {t.pluginActions.unlistConfirm}
        </ConfirmDialog>
      </div>
    </PageContainer>
  )
}
