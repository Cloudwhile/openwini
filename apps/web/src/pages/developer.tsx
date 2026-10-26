import { useEffect, useState } from "react"
import { useParams } from "react-router-dom"
import { Skeleton } from "@workspace/ui/components/skeleton"
import { api, type PermissionInfo } from "@/lib/api"
import { PluginCard } from "@/components/plugin-card"
import { t } from "@/i18n"

interface DeveloperPlugin {
  id: string
  name: string
  version: string
  permissions: string[]
  updatesHalted: boolean
  description?: string
  descriptionEn?: string
}

interface DeveloperPageData {
  developerId: string
  developerName: string
  official: boolean
  plugins: DeveloperPlugin[]
}

export function DeveloperPage() {
  const { developerId } = useParams<{ developerId: string }>()
  // 以 developerId 为键缓存请求结果：切换开发者时旧数据自动失效（无需在 effect 中同步重置）
  const [cache, setCache] = useState<{ id: string; data: DeveloperPageData; error?: string } | null>(null)
  const [known, setKnown] = useState<Record<string, PermissionInfo> | null>(null)

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
    if (!developerId) return
    api
      .get<DeveloperPageData>(`/api/public/developers/${encodeURIComponent(developerId)}`)
      .then((data) => setCache({ id: developerId, data }))
      .catch((e: Error) =>
        setCache({
          id: developerId,
          data: { developerId, developerName: "", official: false, plugins: [] },
          error: e.message,
        }),
      )
  }, [developerId])

  const data = cache && cache.id === developerId ? cache.data : null
  const error = cache && cache.id === developerId ? cache.error : undefined

  if (error) return <div className="text-destructive py-20 text-center text-sm">{error}</div>
  if (!data)
    return (
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3" aria-busy="true">
        {Array.from({ length: 3 }).map((_, i) => (
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
          </div>
        ))}
      </div>
    )

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <h1 className="text-[26px] leading-8 font-semibold tracking-tight">{data.developerName}</h1>
        <span className="text-muted-foreground font-mono text-xs">{data.developerId}</span>
      </div>
      {data.plugins.length === 0 ? (
        <div className="text-muted-foreground py-16 text-center text-sm">{t.common.empty}</div>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {data.plugins.map((p) => (
            <PluginCard
              key={p.id}
              plugin={{ ...p, official: data.official, developerId: data.developerId, developerName: data.developerName }}
              known={known}
            />
          ))}
        </div>
      )}
    </div>
  )
}
