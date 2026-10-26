import type { ReactNode } from "react"
import { AlertTriangle, Ban, Loader2, RefreshCw } from "lucide-react"
import { Button } from "@workspace/ui/components/button"
import { cn } from "@workspace/ui/lib/utils"
import { t } from "@/i18n"

/**
 * 统一页面状态：Loading / Empty / Error / Forbidden。
 * 全站主要列表与详情页共用，禁止用「暂无数据」掩盖网络错误。
 */

export function LoadingState({ label, className }: { label?: string; className?: string }) {
  return (
    <div role="status" aria-live="polite" className={cn("flex flex-col items-center justify-center gap-3 py-16 text-muted-foreground", className)}>
      <Loader2 className="size-6 animate-spin text-primary" aria-hidden />
      <p className="text-sm">{label ?? t.common.loading}</p>
    </div>
  )
}

export function ErrorState({
  message,
  onRetry,
  className,
}: {
  message?: string
  onRetry?: () => void
  className?: string
}) {
  return (
    <div
      role="alert"
      className={cn(
        "flex flex-col items-center justify-center gap-3 rounded-xl border border-destructive/30 bg-destructive/5 py-14 text-center",
        className,
      )}
    >
      <AlertTriangle className="size-7 text-destructive" aria-hidden />
      <div>
        <p className="text-sm font-medium">{t.common.error}</p>
        <p className="text-muted-foreground mt-1 max-w-md text-xs leading-relaxed">{message ?? t.errors.network}</p>
      </div>
      {onRetry ? (
        <Button variant="outline" size="icon" onClick={onRetry} className="mt-1" aria-label={t.common.retry} title={t.common.retry}>
          <RefreshCw className="size-3.5" aria-hidden />
        </Button>
      ) : null}
    </div>
  )
}

export function ForbiddenState({ message }: { message?: string }) {
  return (
    <div
      role="alert"
      className="flex flex-col items-center justify-center gap-3 rounded-xl border border-warning/30 bg-warning/5 py-16 text-center"
    >
      <Ban className="text-warning size-7" aria-hidden />
      <p className="max-w-md text-sm leading-relaxed">{message ?? t.errors.needAdmin}</p>
    </div>
  )
}

export function EmptyState({
  icon,
  title,
  description,
  action,
  className,
}: {
  icon?: ReactNode
  title: string
  description?: string
  action?: ReactNode
  className?: string
}) {
  return (
    <div className={cn("flex flex-col items-center justify-center gap-3 py-16 text-center", className)}>
      <div className="bg-muted text-muted-foreground flex size-12 items-center justify-center rounded-xl" aria-hidden>
        {icon}
      </div>
      <div>
        <p className="text-sm font-medium">{title}</p>
        {description ? <p className="text-muted-foreground mt-1 max-w-sm text-xs leading-relaxed">{description}</p> : null}
      </div>
      {action ? <div className="mt-1">{action}</div> : null}
    </div>
  )
}
