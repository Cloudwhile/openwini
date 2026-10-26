import { useCallback, useEffect, useState } from "react"
import { Users } from "lucide-react"
import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"
import { Dialog } from "@workspace/ui/components/dialog"
import { ConfirmDialog } from "@/components/confirm-dialog"
import { TableSkeleton } from "@/components/table-skeleton"
import { Input } from "@workspace/ui/components/input"
import { Label } from "@workspace/ui/components/label"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@workspace/ui/components/table"
import { Textarea } from "@workspace/ui/components/textarea"
import { api, fmtTime, type Paged } from "@/lib/api"
import { useRequireAdmin } from "@/lib/auth"
import { useToast } from "@/components/toast"
import { Pagination } from "@/components/pagination"
import { EmptyState } from "@/components/page-states"
import { PageContainer, PageHeader } from "@/components/page-header"
import { t } from "@/i18n"

interface AdminDeveloper {
  developerId: string
  displayName: string
  githubLogin: string
  isAdmin: boolean
  isBanned: boolean
  isSystem: boolean
  pluginCount: number
  createdAt: string
}

type Action = "ban" | "unban" | null

export function AdminDevelopers() {
  const { ready, isAdmin, me } = useRequireAdmin()
  const { toast } = useToast()
  const [data, setData] = useState<Paged<AdminDeveloper> | null>(null)
  const [q, setQ] = useState("")
  const [query, setQuery] = useState("")
  const [page, setPage] = useState(1)
  const [action, setAction] = useState<Action>(null)
  const [adminToggle, setAdminToggle] = useState<{ dev: AdminDeveloper; next: boolean } | null>(null)
  const [target, setTarget] = useState<AdminDeveloper | null>(null)
  const [reason, setReason] = useState("")
  const [busy, setBusy] = useState(false)

  const load = useCallback(() => {
    api
      .get<Paged<AdminDeveloper>>(`/api/admin/developers?page=${page}&q=${encodeURIComponent(query)}`)
      .then(setData)
      .catch((e) => toast("error", e.message))
  }, [page, query, toast])

  useEffect(() => {
    if (isAdmin) load()
  }, [isAdmin, load])

  if (!ready) return null
  if (!isAdmin) return <div className="text-destructive py-20 text-center text-sm">{t.errors.needAdmin}</div>

  const openAction = (a: Action, d: AdminDeveloper) => {
    // 确认文案并入主题化弹窗，不再使用原生 window.confirm
    setAction(a)
    setTarget(d)
    setReason("")
  }

  const doAction = async () => {
    if (!action || !target) return
    if (action === "ban" && !reason.trim()) return
    setBusy(true)
    try {
      if (action === "ban") {
        await api.post(`/api/admin/developers/${encodeURIComponent(target.developerId)}/ban`, { reason: reason.trim() })
      } else {
        await api.post(`/api/admin/developers/${encodeURIComponent(target.developerId)}/unban`, { note: reason.trim() || null })
      }
      toast("success", t.misc.operationSuccess)
      setAction(null)
      setTarget(null)
      setReason("")
      load()
    } catch (e) {
      toast("error", (e as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const toggleAdmin = async (d: AdminDeveloper) => {
    const next = !d.isAdmin
    setAdminToggle({ dev: d, next })
  }

  const doToggleAdmin = async () => {
    if (!adminToggle) return
    try {
      await api.put(`/api/admin/developers/${encodeURIComponent(adminToggle.dev.developerId)}/admin`, {
        isAdmin: adminToggle.next,
      })
      toast("success", t.misc.adminUpdated)
      setAdminToggle(null)
      load()
    } catch (e) {
      toast("error", (e as Error).message)
    }
  }

  return (
    <PageContainer>
      <div className="animate-in space-y-5 fade-in duration-300">
        <PageHeader title={t.admin.developers.title} description={t.admin.developers.subtitle} />

        <Input
          type="search"
          aria-label={t.common.search}
          placeholder={t.common.search}
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              setQuery(q)
              setPage(1)
            }
          }}
          className="max-w-xs"
        />

        <div className="bg-card overflow-x-auto rounded-xl border">
          <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t.admin.developers.columns.developer}</TableHead>
              <TableHead>{t.admin.developers.columns.github}</TableHead>
              <TableHead>{t.admin.developers.columns.plugins}</TableHead>
              <TableHead>{t.admin.developers.columns.role}</TableHead>
              <TableHead>{t.admin.developers.columns.status}</TableHead>
              <TableHead>{t.misc.registeredAt}</TableHead>
              <TableHead>{t.admin.developers.columns.actions}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {!data ? (
              <TableSkeleton cols={7} rows={6} widths={[30, 20, 14, 18, 18, 24, 26]} />
            ) : data.items.length === 0 ? (
              <TableRow>
                <TableCell colSpan={7}>
                  <EmptyState icon={<Users className="size-5" />} title={t.admin.developers.empty} />
                </TableCell>
              </TableRow>
            ) : (
              data.items.map((d) => (
                <TableRow key={d.developerId}>
                  <TableCell>
                    <div className="font-mono text-xs">{d.developerId}</div>
                    <div className="text-muted-foreground text-xs">{d.displayName}</div>
                  </TableCell>
                  <TableCell className="font-mono text-xs">{d.githubLogin || "—"}</TableCell>
                  <TableCell className="text-xs">{d.pluginCount}</TableCell>
                  <TableCell>
                    {d.isSystem ? (
                      <Badge variant="outline">{t.admin.developers.systemIdentity}</Badge>
                    ) : d.isAdmin ? (
                      <Badge variant="default">{t.misc.admin}</Badge>
                    ) : (
                      <span className="text-muted-foreground text-xs">{t.misc.developerRole}</span>
                    )}
                  </TableCell>
                  <TableCell>{d.isBanned ? <Badge variant="destructive">{t.misc.banned}</Badge> : <Badge variant="success">{t.misc.normal}</Badge>}</TableCell>
                  <TableCell className="text-muted-foreground text-xs">{fmtTime(d.createdAt)}</TableCell>
                  <TableCell>
                    {d.isSystem ? (
                      <span className="text-muted-foreground text-xs">—</span>
                    ) : (
                      <div className="flex flex-wrap gap-1">
                        {d.isBanned ? (
                          <Button size="xs" variant="outline" onClick={() => openAction("unban", d)}>
                            {t.admin.developers.unban}
                          </Button>
                        ) : (
                          <Button
                            size="xs"
                            variant="destructive"
                            title={d.isAdmin ? t.admin.developers.banAdminBlocked : undefined}
                            disabled={d.isAdmin}
                            onClick={() => openAction("ban", d)}
                          >
                            {t.admin.developers.ban}
                          </Button>
                        )}
                        {d.developerId !== me?.developer?.developerId ? (
                          <Button size="xs" variant="outline" onClick={() => toggleAdmin(d)}>
                            {d.isAdmin ? t.admin.developers.revokeAdmin : t.admin.developers.grantAdmin}
                          </Button>
                        ) : null}
                      </div>
                    )}
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
        </div>

      {data ? <Pagination page={data.page} pageSize={data.pageSize} total={data.total} onPage={setPage} /> : null}

      <Dialog
        open={!!action && !!target}
        onClose={() => {
          setAction(null)
          setTarget(null)
        }}
        title={action === "ban" ? t.admin.developers.banTitle : t.admin.developers.unbanTitle}
        footer={
          <>
            <Button
              variant="outline"
              onClick={() => {
                setAction(null)
                setTarget(null)
              }}
            >
              {t.common.cancel}
            </Button>
            <Button
              variant={action === "ban" ? "destructive" : "default"}
              disabled={busy || (action === "ban" && !reason.trim())}
              onClick={doAction}
            >
              {action === "ban" ? t.admin.developers.ban : t.admin.developers.unban}
            </Button>
          </>
        }
      >
        {target ? (
          <div className="space-y-3 text-sm">
            <div className="font-mono text-xs">{target.developerId}</div>
            <p className="text-muted-foreground leading-relaxed">
              {action === "ban" ? t.admin.developers.banConfirm : t.admin.developers.unbanConfirm}
            </p>
            <div className="space-y-1.5">
              <Label>{action === "ban" ? t.admin.developers.banReasonLabel : t.misc.noteOptional}</Label>
              <Textarea value={reason} rows={3} onChange={(e) => setReason(e.target.value)} />
            </div>
          </div>
        ) : null}
      </Dialog>

      {/* 授予/收回管理员：主题化确认弹窗 */}
      <ConfirmDialog
        open={!!adminToggle}
        onClose={() => setAdminToggle(null)}
        onConfirm={doToggleAdmin}
        title={adminToggle?.next ? t.admin.developers.grantAdmin : t.admin.developers.revokeAdmin}
        confirmText={adminToggle?.next ? t.admin.developers.grantAdmin : t.admin.developers.revokeAdmin}
      >
        <p className="text-muted-foreground text-sm leading-relaxed">
          <span className="font-mono text-xs">{adminToggle?.dev.developerId}</span>
          <span className="mt-1.5 block">
            {adminToggle?.next ? t.admin.developers.grantConfirm : t.admin.developers.revokeConfirm}
          </span>
        </p>
      </ConfirmDialog>
      </div>
    </PageContainer>
  )
}
