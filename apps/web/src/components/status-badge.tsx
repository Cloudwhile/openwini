import { Badge } from "@workspace/ui/components/badge"
import { t, type Lang } from "@/i18n"

/**
 * 展示状态（displayStatus）→ 徽章。
 * 颜色 + 文字双重表达，不只靠颜色；文案渲染时按当前语言解析。
 * returned = 管理员退回修改（区别于审核驳回 rejected），由后端 display_status 推导。
 */
const STATUS_STYLE: Record<
  string,
  { variant: "default" | "secondary" | "destructive" | "outline" | "success" | "warning"; label: (d: Lang) => string }
> = {
  draft: { variant: "outline", label: (d) => d.status.draft },
  pending: { variant: "warning", label: (d) => d.status.pending },
  published: { variant: "success", label: (d) => d.status.published },
  rejected: { variant: "destructive", label: (d) => d.status.rejected },
  returned: { variant: "warning", label: (d) => d.status.returned },
  cancelled: { variant: "secondary", label: (d) => d.status.cancelled },
  stopped: { variant: "destructive", label: (d) => d.status.stopped },
  updates_halted: { variant: "warning", label: (d) => d.status.updates_halted },
  paused: { variant: "secondary", label: (d) => d.status.paused },
  unlisted: { variant: "destructive", label: (d) => d.status.unlisted },
  approved: { variant: "success", label: (d) => d.status.approved },
}

export function StatusBadge({ status }: { status: string }) {
  const s = STATUS_STYLE[status] ?? { variant: "outline" as const, label: null }
  return <Badge variant={s.variant}>{s.label ? s.label(t) : status}</Badge>
}
