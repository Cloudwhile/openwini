import type { ReactNode } from "react"
import { cn } from "cn"
import { Skeleton } from "@workspace/ui/components/skeleton"

/** 后台仪表盘统计卡：图标 + 数值 + 标签；value 传 undefined 时显示骨架扫光 */
export function StatCard({
  icon,
  value,
  label,
  tone = "default",
  className,
}: {
  icon: ReactNode
  value: ReactNode | undefined
  label: ReactNode
  tone?: "default" | "warning" | "success" | "destructive"
  className?: string
}) {
  const tones = {
    default: "bg-primary/10 text-primary",
    warning: "bg-amber-500/10 text-amber-600 dark:text-amber-400",
    success: "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400",
    destructive: "bg-destructive/10 text-destructive",
  }
  return (
    <div
      className={cn(
        "bg-card flex items-center gap-4 rounded-xl border p-5 shadow-sm transition-all hover:shadow-md",
        className,
      )}
    >
      <div className={cn("flex size-11 shrink-0 items-center justify-center rounded-lg", tones[tone])}>
        {icon}
      </div>
      <div className="min-w-0 flex-1">
        {value === undefined ? (
          <Skeleton className="mb-1.5 h-6 w-12" />
        ) : (
          <div className="text-2xl leading-tight font-semibold">{value}</div>
        )}
        <div className="text-muted-foreground truncate text-sm">{label}</div>
      </div>
    </div>
  )
}
