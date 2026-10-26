import { useNavigate } from "react-router-dom"
import { ArrowRight } from "lucide-react"
import { Badge } from "@workspace/ui/components/badge"
import { cn } from "@workspace/ui/lib/utils"
import type { PermissionInfo } from "@/lib/api"
import { PermissionBadges } from "@/components/permission-badge"
import { getLang, t } from "@/i18n"

/** 插件卡片数据：目录条目与开发者主页条目的公共子集 */
export interface PluginCardData {
  id: string
  name: string
  version: string
  description?: string | null
  descriptionEn?: string | null
  permissions: string[]
  updatesHalted?: boolean
  official?: boolean
  developerId?: string
  developerName?: string
}

/** 插件标识首字母瓦片（目录无图标数据时的稳定降级，不伪造图标） */
export function PluginTile({ name }: { name: string }) {
  const letter = (name || "?").slice(0, 1).toUpperCase()
  return (
    <span className="bg-brand-muted text-primary flex size-10 shrink-0 items-center justify-center rounded-lg text-sm font-bold">
      {letter}
    </span>
  )
}

/**
 * 插件卡片：任何需要以卡片表达插件的位置复用（公开目录、开发者主页等）。
 * 整卡可点进入插件详情；开发者名可点进入其主页（阻止冒泡）。
 */
export function PluginCard({
  plugin,
  known,
  className,
  permissionLimit = 4,
}: {
  plugin: PluginCardData
  /** 已知权限表（用于语义展示）；无则按未知权限渲染 */
  known?: Record<string, PermissionInfo> | null
  className?: string
  /** 最多展示的权限徽章数量 */
  permissionLimit?: number
}) {
  const navigate = useNavigate()
  const lang = getLang()
  const go = () => navigate(`/plugins/${plugin.id}`)
  return (
    <article
      role="link"
      tabIndex={0}
      aria-label={plugin.name}
      onClick={go}
      onKeyDown={(e) => {
        if (e.key === "Enter") go()
      }}
      className={cn(
        "group bg-card hover:border-primary/40 flex h-full cursor-pointer flex-col gap-3 rounded-xl border p-5 shadow-sm transition-all duration-200 hover:-translate-y-0.5 hover:shadow-md focus-visible:ring-ring/50 focus-visible:ring-[3px] focus-visible:outline-none",
        className,
      )}
    >
      <div className="flex items-start gap-3">
        <PluginTile name={plugin.name} />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <h2 className="truncate text-[15px] leading-5 font-semibold">{plugin.name}</h2>
            {plugin.updatesHalted ? <Badge variant="warning">{t.catalog.haltedBadge}</Badge> : null}
          </div>
          <p className="text-muted-foreground truncate font-mono text-xs">
            {plugin.id} · v{plugin.version}
          </p>
        </div>
      </div>
      <p className="text-muted-foreground line-clamp-2 min-h-10 text-xs leading-relaxed">
        {(lang === "en" && plugin.descriptionEn) || plugin.description || t.misc.noDescription}
      </p>
      <div className="mt-auto space-y-2.5">
        <PermissionBadges permissions={plugin.permissions.slice(0, permissionLimit)} known={known ?? null} showRisk={false} />
        <div className="text-muted-foreground flex items-center justify-between border-t pt-2.5 text-xs">
          {plugin.developerId && plugin.developerName ? (
            <a
              href={`/developers/${plugin.developerId}`}
              className="hover:text-foreground truncate"
              onClick={(e) => {
                e.preventDefault()
                e.stopPropagation()
                navigate(`/developers/${plugin.developerId}`)
              }}
            >
              {t.catalog.by} {plugin.developerName}
            </a>
          ) : (
            <span />
          )}
          <span className="text-primary inline-flex items-center gap-1 opacity-80 transition-opacity group-hover:opacity-100" aria-hidden>
            {t.catalog.viewDetail}
            <ArrowRight className="size-3" />
          </span>
        </div>
      </div>
    </article>
  )
}
