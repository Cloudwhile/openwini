import { useEffect, type ComponentProps, type ReactNode } from "react"
import { cn } from "cn"

function DialogOverlay({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      className={cn("fixed inset-0 z-50 bg-black/50 backdrop-blur-[2px]", className)}
      {...props}
    />
  )
}

type DialogProps = {
  open: boolean
  onClose: () => void
  title?: ReactNode
  description?: ReactNode
  footer?: ReactNode
  children?: ReactNode
  className?: string
}

/** 受控模态框：Esc 关闭、点击遮罩关闭；危险操作由调用方自行二次确认 */
function Dialog({ open, onClose, title, description, footer, children, className }: DialogProps) {
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose()
    }
    document.addEventListener("keydown", onKey)
    document.body.style.overflow = "hidden"
    return () => {
      document.removeEventListener("keydown", onKey)
      document.body.style.overflow = ""
    }
  }, [open, onClose])

  if (!open) return null
  return (
    <div role="dialog" aria-modal="true">
      <DialogOverlay onClick={onClose} />
      <div className="fixed top-1/2 left-1/2 z-50 w-[calc(100%-2rem)] max-w-lg -translate-x-1/2 -translate-y-1/2">
        <div
          className={cn(
            "bg-background max-h-[85vh] w-full overflow-y-auto rounded-xl border p-6 shadow-lg",
            className,
          )}
        >
          {title ? <div className="mb-1 font-medium">{title}</div> : null}
          {description ? (
            <div className="text-muted-foreground mb-4 text-sm">{description}</div>
          ) : null}
          {children}
          {footer ? <div className="mt-6 flex justify-end gap-2">{footer}</div> : null}
        </div>
      </div>
    </div>
  )
}

export { Dialog }
