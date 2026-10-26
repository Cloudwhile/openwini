import { useEffect, useRef, useState, type ReactNode } from "react"
import { cn } from "cn"

/**
 * 滚动渐入包装：元素进入视口时触发一次 fade + slide 动画。
 * 内容在首屏时同样生效（IntersectionObserver 立即命中）。
 */
export function Reveal({
  children,
  className,
  delay = 0,
}: {
  children: ReactNode
  className?: string
  /** 延迟毫秒数，用于列表交错 */
  delay?: number
}) {
  const ref = useRef<HTMLDivElement>(null)
  // 环境不支持 IntersectionObserver 时直接可见（惰性初始化，避免在 effect 中同步 setState）
  const [shown, setShown] = useState(() => typeof IntersectionObserver === "undefined")

  useEffect(() => {
    const el = ref.current
    if (!el || shown) return
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) {
            setShown(true)
            io.disconnect()
          }
        }
      },
      { rootMargin: "0px 0px -40px 0px", threshold: 0.05 },
    )
    io.observe(el)
    return () => io.disconnect()
  }, [shown])

  return (
    <div
      ref={ref}
      className={cn(
        "transition-all duration-500 ease-out will-change-transform",
        shown ? "translate-y-0 opacity-100" : "translate-y-3 opacity-0",
        className,
      )}
      style={shown ? { transitionDelay: `${delay}ms` } : undefined}
    >
      {children}
    </div>
  )
}
