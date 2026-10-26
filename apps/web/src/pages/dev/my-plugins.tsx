import { useCallback, useEffect, useState } from "react"
import { Link, useNavigate } from "react-router-dom"
import { PackageOpen } from "lucide-react"
import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"
import { Input } from "@workspace/ui/components/input"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@workspace/ui/components/table"
import { api, fmtTime, type DevPlugin, type Paged } from "@/lib/api"
import { useAuth, useRequireAuth } from "@/lib/auth"
import { useToast } from "@/components/toast"
import { StatusBadge } from "@/components/status-badge"
import { TableSkeleton } from "@/components/table-skeleton"
import { Pagination } from "@/components/pagination"
import { ConfirmDialog } from "@/components/confirm-dialog"
import { EmptyState } from "@/components/page-states"
import { SearchableSelect } from "@/components/searchable-select"
import { PageContainer, PageHeader } from "@/components/page-header"
import { t } from "@/i18n"

/** 按展示状态推导「我的插件」行的主操作（P1-04：不再一律显示「编辑」） */
function primaryAction(status: string): { label: string; hint?: string } {
  switch (status) {
    case "draft":
      return { label: t.me.continueEdit }
    case "pending":
      return { label: t.me.viewApplication }
    case "returned":
    case "rejected":
      return { label: t.me.viewFeedback }
    case "published":
    case "updates_halted":
      return { label: t.me.manageVersions }
    case "paused":
      return { label: t.me.viewReason }
    case "stopped":
      return { label: t.me.viewReason }
    case "unlisted":
      return { label: t.me.resubmit }
    default:
      return { label: t.common.view }
  }
}

export function MyPlugins() {
  const { me, ready, logged } = useRequireAuth()
  const { refresh } = useAuth()
  const navigate = useNavigate()
  const { toast } = useToast()
  const [data, setData] = useState<Paged<DevPlugin> | null>(null)
  const [status, setStatus] = useState("")
  const [q, setQ] = useState("")
  const [query, setQuery] = useState("")
  const [page, setPage] = useState(1)
  const [confirmDisconnect, setConfirmDisconnect] = useState(false)
  const [error, setError] = useState("")
  const dev = me?.developer

  const load = useCallback(() => {
    api
      .get<Paged<DevPlugin>>(
        `/api/dev/plugins?page=${page}&status=${encodeURIComponent(status)}&q=${encodeURIComponent(query)}`,
      )
      .then((d) => {
        setData(d)
        setError("")
      })
      .catch((e: Error) => {
        setError(e.message)
        toast("error", e.message)
      })
  }, [page, status, query, toast])

  useEffect(() => {
    if (ready && !logged) navigate("/login")
  }, [ready, logged, navigate])

  useEffect(() => {
    if (logged) load()
  }, [logged, load])

  if (!ready || !logged) return null

  const disconnect = async () => {
    try {
      await api.post("/api/dev/github/disconnect")
      toast("success", t.misc.disconnected)
      await refresh()
    } catch (e) {
      toast("error", (e as Error).message)
    }
  }

  const hasFilter = !!(status || query)

  return (
    <PageContainer>
      <div className="animate-in space-y-5 fade-in duration-300">
        <PageHeader
          title={t.me.title}
          description={t.me.subtitle}
          actions={
            <>
              {dev?.githubConnected ? (
                <>
                  <Badge variant="success">{t.me.githubConnected}</Badge>
                  <Button variant="outline" size="sm" onClick={() => setConfirmDisconnect(true)}>
                    {t.me.disconnect}
                  </Button>
                </>
              ) : (
                <>
                  <Badge variant="destructive">{t.me.githubDisconnected}</Badge>
                  <Button size="sm" onClick={() => (window.location.href = "/api/auth/github/start")}>
                    {t.me.reconnect}
                  </Button>
                </>
              )}
              <Button size="sm" disabled={dev?.isBanned} onClick={() => navigate("/me/plugins/new")}>
                {t.me.newPlugin}
              </Button>
            </>
          }
        />

        {dev?.isBanned ? (
          <div role="alert" className="border-destructive/40 bg-destructive/5 rounded-lg border p-3 text-sm text-destructive">
            {t.me.bannedWarning}
          </div>
        ) : null}

        <div className="flex flex-col gap-2 sm:flex-row">
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
            className="sm:max-w-xs"
          />
          <SearchableSelect
            searchable={false}
            ariaLabel={t.common.filter}
            value={status}
            onChange={(v) => {
              setStatus(v)
              setPage(1)
            }}
            className="sm:max-w-44"
            options={[
              { value: "", label: t.common.all },
              ...Object.entries(t.status).map(([k, v]) => ({ value: k, label: v })),
            ]}
          />
        </div>

        {error ? (
          <div role="alert" className="border-destructive/40 bg-destructive/5 text-destructive rounded-lg border p-3 text-sm">
            {error}
          </div>
        ) : null}

        <div className="bg-card overflow-x-auto rounded-xl border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t.me.columns.plugin}</TableHead>
                <TableHead>{t.me.columns.status}</TableHead>
                <TableHead>{t.me.columns.version}</TableHead>
                <TableHead>{t.me.columns.pendingVersion}</TableHead>
                <TableHead>{t.me.columns.updated}</TableHead>
                <TableHead className="text-right">{t.me.columns.actions}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {!data ? (
                <TableSkeleton cols={6} rows={6} widths={[40, 20, 16, 20, 26, 16]} />
              ) : data.items.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6}>
                    <EmptyState
                      icon={<PackageOpen className="size-5" />}
                      title={hasFilter ? t.common.empty : t.me.empty}
                      description={hasFilter ? t.common.empty : t.me.emptyHint}
                      action={
                        !hasFilter && !dev?.isBanned ? (
                          <Button size="sm" onClick={() => navigate("/me/plugins/new")}>
                            {t.me.newPlugin}
                          </Button>
                        ) : undefined
                      }
                    />
                  </TableCell>
                </TableRow>
              ) : (
                data.items.map((p) => {
                  const action = primaryAction(p.status)
                  return (
                    <TableRow key={p.pluginId}>
                      <TableCell>
                        <Link
                          to={`/me/plugins/${encodeURIComponent(p.pluginId)}`}
                          className="font-mono text-xs hover:underline"
                        >
                          {p.pluginId}
                        </Link>
                      </TableCell>
                      <TableCell>
                        <StatusBadge status={p.status} />
                      </TableCell>
                      <TableCell className="text-xs">{p.version || "—"}</TableCell>
                      <TableCell className="text-xs">
                        {p.pendingVersion ? <Badge variant="warning">{p.pendingVersion}</Badge> : "—"}
                      </TableCell>
                      <TableCell className="text-muted-foreground text-xs">{fmtTime(p.updatedAt)}</TableCell>
                      <TableCell className="text-right">
                        <Button
                          variant="outline"
                          size="xs"
                          onClick={() => navigate(`/me/plugins/${encodeURIComponent(p.pluginId)}`)}
                        >
                          {action.label}
                        </Button>
                      </TableCell>
                    </TableRow>
                  )
                })
              )}
            </TableBody>
          </Table>
        </div>

        {data ? <Pagination page={data.page} pageSize={data.pageSize} total={data.total} onPage={setPage} /> : null}

        <ConfirmDialog
          open={confirmDisconnect}
          onClose={() => setConfirmDisconnect(false)}
          onConfirm={disconnect}
          title={t.me.disconnect}
        >
          {t.me.disconnectWarning}
        </ConfirmDialog>
      </div>
    </PageContainer>
  )
}
