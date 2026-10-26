import type { CSSProperties } from "react"
import { cn } from "cn"

/** 骨架块：扫光动画由 globals.css 的 .skeleton 类提供 */
export function Skeleton({ className, style }: { className?: string; style?: CSSProperties }) {
  return <div data-slot="skeleton" className={cn("skeleton", className)} style={style} />
}

/** 多行文本骨架 */
export function SkeletonText({ lines = 3, className }: { lines?: number; className?: string }) {
  return (
    <div className={cn("space-y-2", className)}>
      {Array.from({ length: lines }).map((_, i) => (
        <Skeleton key={i} className={cn("h-3.5", i === lines - 1 ? "w-2/3" : "w-full")} />
      ))}
    </div>
  )
}
