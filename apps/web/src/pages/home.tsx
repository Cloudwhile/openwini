import { useEffect, useState } from "react"
import { PackageOpen, Search, X } from "lucide-react"
import { Input } from "@workspace/ui/components/input"
import { Badge } from "@workspace/ui/components/badge"
import { Skeleton } from "@workspace/ui/components/skeleton"
import { api, type CatalogItem, type Paged, type PermissionInfo } from "@/lib/api"
import { Pagination } from "@/components/pagination"
import { PluginCard } from "@/components/plugin-card"
import { EmptyState, ErrorState } from "@/components/page-states"
import { Reveal } from "@/components/reveal"
import { t } from "@/i18n"

export function Home() {
  const [q, setQ] = useState("")
  const [query, setQuery] = useState("")
  const [developer, setDeveloper] = useState("")
  const [page, setPage] = useState(1)
  const [data, setData] = useState<Paged<CatalogItem> | null>(null)
  const [known, setKnown] = useState<Record<string, PermissionInfo> | null>(null)
  const [closed, setClosed] = useState(false)
  const [error, setError] = useState("")
  const [loading, setLoading] = useState(true)
  const [retry, setRetry] = useState(0)

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

  useEffect(() => {
    let active = true
    const timer = setTimeout(() => {
      setError("")
      setLoading(true)
      api
        .get<Paged<CatalogItem>>(
          `/api/public/catalog?page=${page}${query ? `&q=${encodeURIComponent(query)}` : ""}${developer ? `&developer=${encodeURIComponent(developer)}` : ""}`,
        )
        .then((d) => {
          if (!active) return
          setData(d)
          setClosed(false)
        })
        .catch((e: { status?: number; message?: string }) => {
          if (!active) return
          if (e.status === 403) setClosed(true)
          else setError(String(e.message))
        })
        .finally(() => { if (active) setLoading(false) })
    }, 250)
    return () => {
      active = false
      clearTimeout(timer)
    }
  }, [query, developer, page, retry])

  if (closed) {
    return (
      <div className="py-20">
        <EmptyState
          icon={<PackageOpen className="size-5" />}
          title={t.catalog.closedNotice}
          description={t.catalog.closedHint}
        />
      </div>
    )
  }

  const hasFilter = !!(query || developer)

  return (
    <div className="space-y-8">
      {/* 页头：标题 + 一句话说明 + 搜索 */}
      <section className="animate-in fade-in slide-in-from-bottom-2 fill-mode-backwards space-y-5 duration-500">
        <div className="space-y-2">
          <h1 className="text-[26px] font-semibold tracking-tight">{t.catalog.title}</h1>
        </div>
        <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
          <div className="relative sm:max-w-md sm:flex-1">
            <Search
              className="text-muted-foreground pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2"
              aria-hidden
            />
            <Input
              type="search"
              aria-label={t.catalog.searchLabel}
              placeholder={t.catalog.searchPlaceholder}
              value={q}
              onChange={(e) => {
                setQ(e.target.value)
                setPage(1)
                setQuery(e.target.value)
              }}
              className="h-10 pl-9"
            />
          </div>
          {data && data.total > 0 ? (
            <Badge variant="secondary" className="w-fit font-mono text-xs">
              {t.common.total(data.total)}
            </Badge>
          ) : null}
        </div>
        {developer ? (
          <button
            className="text-muted-foreground hover:text-foreground inline-flex items-center gap-1 rounded-md bg-muted px-2.5 py-1 text-xs transition-colors"
            onClick={() => {
              setDeveloper("")
              setPage(1)
            }}
          >
            {t.catalog.developerFilter}: {developer}
            <X className="size-3" aria-hidden />
          </button>
        ) : null}
      </section>

      {error ? <ErrorState message={error} onRetry={() => setRetry((value) => value + 1)} /> : null}

      {error ? null : loading && !data ? (
        <CatalogSkeleton />
      ) : data && data.items.length === 0 ? (
        <EmptyState
          icon={<PackageOpen className="size-5" />}
          title={t.catalog.empty}
          description={hasFilter ? t.catalog.emptyHint : t.catalog.subtitle}
          className="animate-in fade-in duration-500"
        />
      ) : data ? (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3" aria-busy={loading}>
          {data.items.map((p, i) => (
            <Reveal key={p.id} delay={Math.min(i, 11) * 40}>
              <PluginCard plugin={p} known={known} />
            </Reveal>
          ))}
        </div>
      ) : null}

      {data && !error ? <Pagination page={data.page} pageSize={data.pageSize} total={data.total} onPage={setPage} /> : null}
    </div>
  )
}

/** 目录加载骨架：网格卡片占位 + 扫光 */
function CatalogSkeleton() {
  return (
    <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3" aria-busy="true">
      {Array.from({ length: 6 }).map((_, i) => (
        <div key={i} className="bg-card space-y-3 rounded-xl border p-5">
          <div className="flex items-start gap-3">
            <Skeleton className="size-10 rounded-lg" />
            <div className="flex-1 space-y-2">
              <Skeleton className="h-4 w-2/5" />
              <Skeleton className="h-3 w-1/2" />
            </div>
          </div>
          <Skeleton className="h-3 w-full" />
          <Skeleton className="h-3 w-4/5" />
          <div className="flex gap-1.5 pt-1">
            <Skeleton className="h-5 w-16 rounded-md" />
            <Skeleton className="h-5 w-20 rounded-md" />
          </div>
        </div>
      ))}
    </div>
  )
}
