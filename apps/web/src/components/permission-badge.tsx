import { Badge } from "@workspace/ui/components/badge"
import { t } from "@/i18n"
import type { PermissionInfo } from "@/lib/api"

/**
 * 权限徽章。
 * showRisk=true（默认，审核/提交场景）：高危权限红色显著标注（颜色 + 文字双重表达）。
 * showRisk=false（公开页面）：已发布插件均通过人工审核，仅展示权限名，不带风险标注。
 */
export function PermissionBadges({
  permissions,
  known,
  showRisk = true,
}: {
  permissions: string[]
  known: Record<string, PermissionInfo> | null
  showRisk?: boolean
}) {
  if (permissions.length === 0) {
    return <span className="text-muted-foreground text-xs">{t.misc.noPermissions}</span>
  }
  if (!showRisk) {
    return (
      <span className="inline-flex flex-wrap gap-1">
        {permissions.map((p) => (
          <Badge key={p} variant="secondary">
            {p}
          </Badge>
        ))}
      </span>
    )
  }
  return (
    <span className="inline-flex flex-wrap gap-1">
      {permissions.map((p) => {
        const risk = known?.[p]?.risk ?? "unknown"
        if (risk === "high") {
          return (
            <Badge key={p} variant="destructive" title={t.permission.highRiskNote}>
              {p} · {t.permission.risk.high}
            </Badge>
          )
        }
        if (risk === "medium") {
          return (
            <Badge key={p} variant="warning">
              {p}
            </Badge>
          )
        }
        if (risk === "unknown") {
          return (
            <Badge key={p} variant="outline" className="border-destructive/50 text-destructive">
              {p} · {t.permission.unknown}
            </Badge>
          )
        }
        return (
          <Badge key={p} variant="secondary">
            {p}
          </Badge>
        )
      })}
    </span>
  )
}
