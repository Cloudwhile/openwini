import { useEffect, useState } from "react"
import { Link, useParams } from "react-router-dom"
import { ArrowLeft, BookOpen, Check, Code2, Copy, ExternalLink, Home as HomeIcon, Info, PackageSearch, PauseCircle, ShieldAlert } from "lucide-react"
import { Card, CardContent, CardHeader, CardTitle } from "@workspace/ui/components/card"
import { Badge } from "@workspace/ui/components/badge"
import { Button } from "@workspace/ui/components/button"
import { Skeleton } from "@workspace/ui/components/skeleton"
import { api, fmtTime, type PluginDetail } from "@/lib/api"
import { Markdown } from "@/components/markdown"
import { PermissionBadges } from "@/components/permission-badge"
import { EmptyState } from "@/components/page-states"
import { getLang, t } from "@/i18n"

export function PluginDetailPage() {
  const { pluginId } = useParams<{ pluginId: string }>()
  // 以 pluginId 为键缓存请求结果：切换插件时旧数据自动失效（无需在 effect 中同步重置）
  const [cache, setCache] = useState<{
    id: string
    data?: PluginDetail
    error?: string
    notFound?: boolean
  } | null>(null)
  const [known, setKnown] = useState<Record<string, { name: string; risk: string; description: string }> | null>(null)

  useEffect(() => {
    api
      .get<Record<string, { name: string; risk: string; description: string }[]>>("/api/public/permissions")
      .then((r) => {
        const m: Record<string, { name: string; risk: string; description: string }> = {}
        for (const p of r.permissions ?? []) m[p.name] = p
        setKnown(m)
      })
      .catch(() => {})
  }, [])

  useEffect(() => {
    if (!pluginId) return
    api
      .get<PluginDetail>(`/api/public/plugins/${encodeURIComponent(pluginId)}`)
      .then((data) => setCache({ id: pluginId, data }))
      .catch((e: { status?: number; message?: string }) =>
        setCache({
          id: pluginId,
          notFound: e.status === 404,
          error: e.status === 404 ? undefined : String(e.message),
        }),
      )
  }, [pluginId])

  const detail = cache && cache.id === pluginId ? cache.data : null
  const error = cache && cache.id === pluginId ? cache.error : undefined
  const notFound = (cache && cache.id === pluginId && !!cache.notFound) || false
  // 安装标识复制反馈
  const [copied, setCopied] = useState(false)
  useEffect(() => {
    if (!copied) return
    const timer = setTimeout(() => setCopied(false), 1500)
    return () => clearTimeout(timer)
  }, [copied])

  const copyId = async () => {
    if (!detail) return
    try {
      await navigator.clipboard.writeText(detail.id)
      setCopied(true)
    } catch {
      // 剪贴板不可用（非安全上下文）：退化为选中文本，用户仍可手动复制
      window.getSelection()?.selectAllChildren(document.getElementById("plugin-install-id")!)
    }
  }

  if (notFound) {
    return (
      <EmptyState
        icon={<PackageSearch className="size-5" />}
        title={t.detail.notFound}
        description={t.detail.notFoundHint}
        action={
          <Button variant="outline" size="sm" onClick={() => (window.location.href = "/")}>
            <ArrowLeft className="size-3.5" />
            {t.detail.backToCatalog}
          </Button>
        }
      />
    )
  }
  if (error) return <div className="text-destructive py-20 text-center text-sm">{error}</div>

  if (!detail)
    return (
      <div className="grid gap-6 lg:grid-cols-[1fr_320px]" aria-busy="true">
        <div className="space-y-6">
          <div className="space-y-3">
            <Skeleton className="h-8 w-1/3" />
            <Skeleton className="h-3.5 w-1/2" />
            <Skeleton className="h-3.5 w-1/4" />
          </div>
          <div className="bg-card space-y-3 rounded-xl border p-6">
            <Skeleton className="h-4 w-24" />
            <Skeleton className="h-3.5 w-full" />
            <Skeleton className="h-3.5 w-11/12" />
            <Skeleton className="h-3.5 w-3/5" />
          </div>
        </div>
        <div className="space-y-4">
          <div className="bg-card space-y-3 rounded-xl border p-5">
            <Skeleton className="h-4 w-20" />
            <Skeleton className="h-5 w-24 rounded-md" />
            <Skeleton className="h-5 w-28 rounded-md" />
          </div>
          <div className="bg-card space-y-3 rounded-xl border p-5">
            <Skeleton className="h-4 w-16" />
            <Skeleton className="h-3.5 w-3/4" />
            <Skeleton className="h-3.5 w-2/3" />
          </div>
        </div>
      </div>
    )

  return (
    <div className="animate-in grid gap-6 fade-in slide-in-from-bottom-2 duration-300 lg:grid-cols-[1fr_320px]">
      <div className="min-w-0 space-y-6">
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <h1 className="text-[26px] leading-8 font-semibold tracking-tight">{detail.name}</h1>
            {detail.status === "stopped" ? (
              <Badge variant="destructive">
                <ShieldAlert className="mr-1 size-3" aria-hidden />
                {t.status.stopped}
              </Badge>
            ) : null}
            {detail.status === "updates_halted" ? (
              <Badge variant="warning">
                <Info className="mr-1 size-3" aria-hidden />
                {t.status.updates_halted}
              </Badge>
            ) : null}
            {detail.status === "paused" ? (
              <Badge variant="secondary">
                <PauseCircle className="mr-1 size-3" aria-hidden />
                {t.status.paused}
              </Badge>
            ) : null}
          </div>
          <div className="text-muted-foreground flex flex-wrap items-center gap-x-2 font-mono text-xs">
            <span>{detail.id}</span>
            <span aria-hidden>·</span>
            <span>v{detail.version}</span>
            <span aria-hidden>·</span>
            <span>
              {t.detail.updatedAt} {fmtTime(detail.updatedAt)}
            </span>
          </div>
          <div className="text-muted-foreground text-sm">
            {t.detail.developer}：
            <Link className="text-foreground hover:underline" to={`/developers/${detail.developerId}`}>
              {detail.developerName}
            </Link>
          </div>
        </div>

        {/* 状态提示条：语义色 + 图标，不只靠颜色 */}
        {detail.stoppedNotice ? (
          <div role="alert" className="border-destructive/40 bg-destructive/5 text-destructive flex items-start gap-2.5 rounded-lg border p-3.5 text-sm leading-relaxed">
            <ShieldAlert className="mt-0.5 size-4 shrink-0" aria-hidden />
            <span>
              <strong className="font-medium">{t.detail.stoppedNotice}</strong>
              {detail.stoppedNotice}
            </span>
          </div>
        ) : null}
        {detail.pausedNotice ? (
          <div role="status" className="border-warning/40 bg-warning/5 flex items-start gap-2.5 rounded-lg border p-3.5 text-sm leading-relaxed">
            <PauseCircle className="text-warning mt-0.5 size-4 shrink-0" aria-hidden />
            <span>{detail.pausedNotice}</span>
          </div>
        ) : null}
        {detail.updatesHaltedNotice ? (
          <div role="status" className="border-warning/40 bg-warning/5 flex items-start gap-2.5 rounded-lg border p-3.5 text-sm leading-relaxed">
            <Info className="text-warning mt-0.5 size-4 shrink-0" aria-hidden />
            <span>
              <strong className="font-medium">{t.detail.haltedNotice}</strong>
              {detail.updatesHaltedNotice}
            </span>
          </div>
        ) : null}

        <Card>
          <CardHeader>
            <CardTitle>{t.detail.description}</CardTitle>
          </CardHeader>
          <CardContent>
            {(() => {
              const desc = (getLang() === "en" && detail.descriptionEn) || detail.description
              return desc ? (
                <Markdown>{desc}</Markdown>
              ) : (
                <span className="text-muted-foreground text-sm">{t.misc.noDescription}</span>
              )
            })()}
          </CardContent>
        </Card>
      </div>

      <aside className="space-y-4">
        <Card className="gap-3 py-4">
          <CardHeader>
            <CardTitle className="text-sm">{t.detail.permissions}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2.5">
            <PermissionBadges
              permissions={detail.permissions.map((p) => p.name)}
              known={known as never}
              showRisk={false}
            />
            {/* 权限语义说明：已知权限给出风险等级与用途 */}
            <ul className="space-y-1.5 pt-1">
              {detail.permissions.map((p) => {
                const k = known?.[p.name]
                return (
                  <li key={p.name} className="text-muted-foreground text-xs leading-relaxed">
                    <span className="text-foreground font-mono">{p.name}</span>
                    {k ? (
                      <span className="ml-1.5">
                        ({t.permission.risk[k.risk as keyof typeof t.permission.risk] ?? t.permission.unknown}) {k.description}
                      </span>
                    ) : null}
                  </li>
                )
              })}
            </ul>
            <p className="text-muted-foreground border-t pt-2 text-xs">{t.detail.permissionsDesc}</p>
          </CardContent>
        </Card>

        <Card className="gap-3 py-4">
          <CardHeader>
            <CardTitle className="text-sm">{t.misc.sources}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2 text-sm">
            {/* 来源入口顺序固定：仓库 → 主页 → 文档 */}
            <a
              href={detail.repositoryUrl}
              target="_blank"
              rel="noreferrer noopener"
              className="hover:bg-muted flex items-center gap-2 rounded-md px-1.5 py-1 transition-colors hover:underline"
            >
              <Code2 className="size-4" /> {t.detail.repository}
              <ExternalLink className="text-muted-foreground size-3" aria-hidden />
            </a>
            {detail.homepageUrl ? (
              <a
                href={detail.homepageUrl}
                target="_blank"
                rel="noreferrer noopener"
                className="hover:bg-muted flex items-center gap-2 rounded-md px-1.5 py-1 transition-colors hover:underline"
              >
                <HomeIcon className="size-4" /> {t.detail.homepage}
                <ExternalLink className="text-muted-foreground size-3" aria-hidden />
              </a>
            ) : null}
            {detail.docsUrl ? (
              <a
                href={detail.docsUrl}
                target="_blank"
                rel="noreferrer noopener"
                className="hover:bg-muted flex items-center gap-2 rounded-md px-1.5 py-1 transition-colors hover:underline"
              >
                <BookOpen className="size-4" /> {t.detail.docs}
                <ExternalLink className="text-muted-foreground size-3" aria-hidden />
              </a>
            ) : null}
            <p className="text-muted-foreground pt-1 text-xs">{t.detail.sourceOrderNote}</p>
          </CardContent>
        </Card>

        <Card className="gap-3 py-4">
          <CardHeader>
            <CardTitle className="text-sm">{t.detail.install}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            <p className="text-muted-foreground text-xs leading-relaxed">{t.detail.installDesc}</p>
            <button
              type="button"
              id="plugin-install-id"
              onClick={copyId}
              title={copied ? t.common.copied : t.common.copy}
              aria-label={`${t.common.copy}: ${detail.id}`}
              className="bg-muted hover:border-primary/40 text-muted-foreground hover:text-foreground flex w-full items-center justify-between gap-2 rounded-md border border-transparent px-2.5 py-1.5 font-mono text-xs transition-colors"
            >
              <span className="truncate">{detail.id}</span>
              {copied ? (
                <span className="text-success inline-flex shrink-0 items-center gap-1 text-[11px]" role="status">
                  <Check className="size-3.5" aria-hidden />
                  {t.common.copied}
                </span>
              ) : (
                <Copy className="size-3.5 shrink-0 opacity-60" aria-hidden />
              )}
            </button>
          </CardContent>
        </Card>
      </aside>
    </div>
  )
}
