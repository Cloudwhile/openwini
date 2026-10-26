import { useCallback, useEffect, useRef, useState } from "react"
import { createPortal } from "react-dom"
import { useNavigate } from "react-router-dom"
import { MoreHorizontal } from "lucide-react"
import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"
import { Dialog } from "@workspace/ui/components/dialog"
import { Input } from "@workspace/ui/components/input"
import { Label } from "@workspace/ui/components/label"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@workspace/ui/components/table"
import { Textarea } from "@workspace/ui/components/textarea"
import { api, fmtTime, type Paged } from "@/lib/api"
import { useRequireAdmin } from "@/lib/auth"
import { useToast } from "@/components/toast"
import { Pagination } from "@/components/pagination"
import { StatusBadge } from "@/components/status-badge"
import { TableSkeleton } from "@/components/table-skeleton"
import { EmptyState } from "@/components/page-states"
import { SearchableSelect } from "@/components/searchable-select"
import { PageContainer, PageHeader } from "@/components/page-header"
import { t } from "@/i18n"

interface AdminPlugin {
  pluginId: string
  developerId: string
  developerName: string
  status: string
  displayStatus: string
  version: string
  repositoryUrl: string
  updatesHalted: boolean
  stopReason?: string | null
  updatesHaltedReason?: string | null
  official: boolean
  updatedAt: string
}

type Action = "stop" | "unpublish" | "halt" | "resume" | null

interface RowMenuItem {
  key: string
  label: string
  destructive?: boolean
}

/**
 * 表格行操作菜单：Portal 到 body + fixed 定位（右对齐触发按钮，贴近视口底部时向上翻转）。
 * 滚动/缩放/Esc 时关闭。避免被表格 overflow 容器裁剪。
 */
function RowMenu({
  open,
  onToggle,
  onClose,
  items,
  onSelect,
}: {
  open: boolean
  onToggle: () => void
  onClose: () => void
  items: RowMenuItem[]
  onSelect: (key: string) => void
}) {
  const anchorRef = useRef<HTMLSpanElement>(null)
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null)
  const MENU_W = 192 // w-48

  useEffect(() => {
    if (!open) return
    const rect = anchorRef.current?.getBoundingClientRect()
    if (!rect) return
    const left = Math.max(8, Math.min(rect.right - MENU_W, window.innerWidth - MENU_W - 8))
    // 近似菜单高度（3~4 项 * 34px + 内边距），放不下时向上翻转
    const estimatedH = items.length * 34 + 8
    const below = rect.bottom + 4
    const top = below + estimatedH > window.innerHeight - 8 ? Math.max(8, rect.top - estimatedH - 4) : below
    setPos({ top, left })
    const close = () => onClose()
    window.addEventListener("scroll", close, true)
    window.addEventListener("resize", close)
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose()
    }
    document.addEventListener("keydown", onKey)
    return () => {
      window.removeEventListener("scroll", close, true)
      window.removeEventListener("resize", close)
      document.removeEventListener("keydown", onKey)
    }
  }, [open, items.length, onClose])

  if (items.length === 0) return <span className="text-muted-foreground text-xs">—</span>
  return (
    <span className="inline-block" ref={anchorRef}>
      <Button
        variant="outline"
        size="icon-xs"
        aria-label={t.admin.plugins.columns.actions}
        aria-expanded={open}
        aria-haspopup="menu"
        onClick={onToggle}
      >
        <MoreHorizontal className="size-3.5" />
      </Button>
      {open && pos
        ? createPortal(
            <>
              <div className="fixed inset-0 z-40" aria-hidden onClick={onClose} />
              <div
                role="menu"
                style={{ top: pos.top, left: pos.left, width: MENU_W }}
                className="bg-popover animate-in fade-in zoom-in-95 fixed z-50 overflow-hidden rounded-lg border p-1 shadow-lg duration-150"
              >
                {items.map((it) => (
                  <button
                    key={it.key}
                    role="menuitem"
                    className={
                      it.destructive
                        ? "text-destructive hover:bg-destructive/10 flex w-full items-center rounded-md px-2.5 py-1.5 text-left text-xs transition-colors"
                        : "hover:bg-muted flex w-full items-center rounded-md px-2.5 py-1.5 text-left text-xs transition-colors"
                    }
                    onClick={() => onSelect(it.key)}
                  >
                    {it.label}
                  </button>
                ))}
              </div>
            </>,
            document.body,
          )
        : null}
    </span>
  )
}

/** 目录可见性：仅 published（含冻结更新）在目录与索引中 */
function VisibilityBadge({ status }: { status: string }) {
  return status === "published" ? (
    <Badge variant="secondary">{t.admin.plugins.visibilityListed}</Badge>
  ) : (
    <Badge variant="outline" className="text-muted-foreground">
      {t.admin.plugins.visibilityHidden}
    </Badge>
  )
}

export function AdminPlugins() {
  const { ready, isAdmin } = useRequireAdmin()
  const { toast } = useToast()
  const navigate = useNavigate()

  const [data, setData] = useState<Paged<AdminPlugin> | null>(null)
  const [q, setQ] = useState("")
  const [query, setQuery] = useState("")
  const [status, setStatus] = useState("")
  const [page, setPage] = useState(1)
  const [action, setAction] = useState<Action>(null)
  const [target, setTarget] = useState<AdminPlugin | null>(null)
  const [reason, setReason] = useState("")
  const [busy, setBusy] = useState(false)
  const [menuFor, setMenuFor] = useState<string | null>(null)

  const load = useCallback(() => {
    api
      .get<Paged<AdminPlugin>>(
        `/api/admin/plugins?page=${page}&q=${encodeURIComponent(query)}&status=${encodeURIComponent(status)}`,
      )
      .then(setData)
      .catch((e: Error) => toast("error", e.message))
  }, [page, query, status, toast])

  useEffect(() => {
    if (isAdmin) load()
  }, [isAdmin, load])

  if (!ready) return null
  if (!isAdmin) return <div className="text-destructive py-20 text-center text-sm">{t.errors.needAdmin}</div>

  const doAction = async () => {
    if (!action || !target) return
    if (action !== "resume" && !reason.trim()) return
    setBusy(true)
    try {
      if (action === "resume") {
        await api.post(`/api/admin/plugins/${encodeURIComponent(target.pluginId)}/resume`, {
          note: reason.trim() || null,
        })
      } else {
        await api.post(`/api/admin/plugins/${encodeURIComponent(target.pluginId)}/${action}`, {
          reason: reason.trim(),
        })
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

  const openAction = (a: Action, p: AdminPlugin) => {
    setMenuFor(null)
    setAction(a)
    setTarget(p)
    setReason("")
  }

  const actionDialogMeta = {
    stop: {
      title: t.admin.plugins.stopTitle,
      confirm: t.admin.plugins.stopConfirm,
      reasonLabel: t.admin.plugins.stopReasonLabel,
    },
    halt: {
      title: t.admin.plugins.haltTitle,
      confirm: t.admin.plugins.haltConfirm,
      reasonLabel: t.admin.plugins.haltReasonLabel,
    },
    unpublish: {
      title: t.admin.plugins.unpublishTitle,
      confirm: t.admin.plugins.unpublishConfirm,
      reasonLabel: t.admin.plugins.unpublishReasonLabel,
    },
    resume: {
      title: t.admin.plugins.resumeTitle,
      confirm: t.admin.plugins.resumeConfirm,
      reasonLabel: t.misc.noteOptional,
    },
  }

  /** 行内操作菜单（P1-06：危险操作收进菜单，主表格只保留一个入口）。
   *  菜单经 Portal 渲染到 body 并用 fixed 定位锚定按钮：
   *  否则会被表格的 overflow-x-auto 容器裁剪，只能在表格内部展开。 */
  function RowActions({ p }: { p: AdminPlugin }) {
    const items: { key: Exclude<Action, null> | "officialVersion"; label: string; destructive?: boolean }[] = []
    if (p.status === "published" && !p.updatesHalted) {
      items.push({ key: "unpublish", label: t.admin.plugins.unpublish })
      items.push({ key: "halt", label: t.admin.plugins.halt })
      items.push({ key: "stop", label: t.admin.plugins.stop, destructive: true })
    }
    if (p.status === "stopped" || (p.status === "published" && p.updatesHalted)) {
      items.push({ key: "resume", label: p.status === "stopped" ? t.admin.plugins.resumePlugin : t.admin.plugins.resumeUpdates })
    }
    if (p.official && p.status === "published") {
      items.push({ key: "officialVersion", label: t.misc.newVersionShort })
    }
    const open = menuFor === p.pluginId
    return (
      <RowMenu
        open={open}
        onToggle={() => setMenuFor(open ? null : p.pluginId)}
        onClose={() => setMenuFor(null)}
        items={items}
        onSelect={(key) => {
          if (key === "officialVersion") {
            setMenuFor(null)
            navigate(`/admin/plugins/${encodeURIComponent(p.pluginId)}/official-version`)
          } else {
            openAction(key as Action, p)
          }
        }}
      />
    )
  }

  return (
    <PageContainer>
      <div className="animate-in space-y-5 fade-in duration-300">
        <PageHeader
          title={t.admin.plugins.title}
          description={t.admin.plugins.subtitle}
          actions={
            <Button size="sm" onClick={() => navigate("/admin/official/new")}>
              {t.admin.plugins.officialCreate}
            </Button>
          }
        />

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
              ...["draft", "pending", "published", "rejected", "returned", "cancelled", "stopped", "paused", "unlisted"].map(
                (s) => ({ value: s, label: (t.status as Record<string, string>)[s] ?? s }),
              ),
            ]}
          />
        </div>

        <div className="bg-card overflow-x-auto rounded-xl border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t.admin.plugins.columns.plugin}</TableHead>
                <TableHead>{t.admin.plugins.columns.developer}</TableHead>
                <TableHead>{t.admin.plugins.columns.status}</TableHead>
                <TableHead>{t.admin.plugins.columns.visibility}</TableHead>
                <TableHead>{t.admin.plugins.columns.version}</TableHead>
                <TableHead>{t.admin.plugins.columns.repo}</TableHead>
                <TableHead>{t.admin.plugins.columns.updated}</TableHead>
                <TableHead className="text-right">{t.admin.plugins.columns.actions}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {!data ? (
                <TableSkeleton cols={8} rows={6} widths={[30, 18, 14, 12, 10, 22, 18, 8]} />
              ) : data.items.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={8}>
                    <EmptyState title={t.admin.plugins.empty} description={t.admin.plugins.emptyHint} />
                  </TableCell>
                </TableRow>
              ) : (
                data.items.map((p) => (
                  <TableRow key={p.pluginId}>
                    <TableCell>
                      <span className="font-mono text-xs">{p.pluginId}</span>
                      {p.official ? (
                        <Badge variant="default" className="ml-1.5">
                          {t.catalog.officialBadge}
                        </Badge>
                      ) : null}
                    </TableCell>
                    <TableCell>
                      <span className="font-mono text-xs">{p.developerId}</span>
                    </TableCell>
                    <TableCell>
                      <StatusBadge status={p.displayStatus} />
                    </TableCell>
                    <TableCell>
                      <VisibilityBadge status={p.status} />
                    </TableCell>
                    <TableCell className="font-mono text-xs">{p.version || "—"}</TableCell>
                    <TableCell className="max-w-44 truncate text-xs">
                      <a href={p.repositoryUrl} target="_blank" rel="noreferrer noopener" className="hover:underline">
                        {p.repositoryUrl.replace("https://github.com/", "")}
                      </a>
                    </TableCell>
                    <TableCell className="text-muted-foreground text-xs">{fmtTime(p.updatedAt)}</TableCell>
                    <TableCell className="text-right">
                      <RowActions p={p} />
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>

        {data ? <Pagination page={data.page} pageSize={data.pageSize} total={data.total} onPage={setPage} /> : null}

        {/* 停止 / 退回 / 停止更新 / 恢复：确认 + 原因 + 影响说明 */}
        <Dialog
          open={!!action && !!target}
          onClose={() => {
            setAction(null)
            setTarget(null)
          }}
          title={action ? actionDialogMeta[action].title : ""}
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
                variant={action === "resume" ? "default" : "destructive"}
                disabled={busy || (action !== "resume" && !reason.trim())}
                onClick={doAction}
              >
                {action ? actionDialogMeta[action].title : ""}
              </Button>
            </>
          }
        >
          {target && action ? (
            <div className="space-y-3 text-sm">
              <div className="font-mono text-xs">
                {target.pluginId} · {target.developerId}
              </div>
              <p className="text-muted-foreground leading-relaxed">{actionDialogMeta[action].confirm}</p>
              {action === "stop" ? (
                <p className="text-destructive text-xs leading-relaxed">{t.admin.plugins.stopWarning}</p>
              ) : null}
              <div className="space-y-1.5">
                <Label htmlFor="action-reason">{actionDialogMeta[action].reasonLabel}</Label>
                <Textarea
                  id="action-reason"
                  value={reason}
                  rows={3}
                  placeholder={action === "resume" ? t.misc.noteOptional : undefined}
                  onChange={(e) => setReason(e.target.value)}
                />
              </div>
            </div>
          ) : null}
        </Dialog>
      </div>
    </PageContainer>
  )
}
