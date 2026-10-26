import type { ReactNode } from "react"
import { cn } from "@workspace/ui/lib/utils"

/**
 * 页面容器：后台统一两种内容宽度。
 * - wide  （≈1280px）：概览、列表、队列、审计
 * - narrow（≈800px）：提交、编辑、设置等表单页，整体居中
 */
export function PageContainer({
  width = "wide",
  className,
  children,
}: {
  width?: "wide" | "narrow"
  className?: string
  children: ReactNode
}) {
  return (
    <div
      className={cn(
        "mx-auto w-full",
        width === "narrow" ? "max-w-[800px]" : "max-w-[1280px]",
        className,
      )}
    >
      {children}
    </div>
  )
}

/** 页头：标题 + 说明 + 右侧操作区，全站统一（页面标题 26px/650） */
export function PageHeader({
  title,
  description,
  actions,
  className,
}: {
  title: ReactNode
  description?: ReactNode
  actions?: ReactNode
  className?: string
}) {
  return (
    <div className={cn("mb-6 flex flex-wrap items-start justify-between gap-x-6 gap-y-3", className)}>
      <div className="min-w-0">
        <h1 className="text-[26px] leading-8 font-semibold tracking-tight">{title}</h1>
        {description ? <p className="text-muted-foreground mt-1.5 text-sm leading-relaxed">{description}</p> : null}
      </div>
      {actions ? <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div> : null}
    </div>
  )
}

/** 表单分区：标题 + 说明 + 内容（设置页/表单页统一骨架）；flush 用于内嵌表格 */
export function FormSection({
  title,
  description,
  children,
  className,
  flush,
}: {
  title: ReactNode
  description?: ReactNode
  children: ReactNode
  className?: string
  flush?: boolean
}) {
  return (
    <section className={cn("rounded-xl border bg-card", className)}>
      <div className="border-b px-6 py-4">
        <h2 className="text-base font-semibold">{title}</h2>
        {description ? <p className="text-muted-foreground mt-0.5 text-xs leading-relaxed">{description}</p> : null}
      </div>
      <div className={flush ? "" : "px-6 py-5"}>{children}</div>
    </section>
  )
}

/** 统一表单字段行：标签 + 控件 + 帮助/错误信息 */
export function FormField({
  label,
  htmlFor,
  help,
  error,
  required,
  children,
  className,
}: {
  label: ReactNode
  htmlFor?: string
  help?: ReactNode
  error?: ReactNode
  required?: boolean
  children: ReactNode
  className?: string
}) {
  return (
    <div className={cn("grid gap-2", className)}>
      <label htmlFor={htmlFor} className="text-sm leading-5 font-medium">
        {label}
        {required ? <span className="text-destructive ml-0.5" aria-hidden>*</span> : null}
      </label>
      {children}
      {error ? (
        <p className="text-destructive text-xs leading-relaxed" role="alert">
          {error}
        </p>
      ) : help ? (
        <p className="text-muted-foreground text-xs leading-relaxed">{help}</p>
      ) : null}
    </div>
  )
}
