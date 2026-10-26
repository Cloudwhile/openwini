import { useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react"
import { Check, ChevronsUpDown, Search, X } from "lucide-react"
import { Button } from "@workspace/ui/components/button"
import { Input } from "@workspace/ui/components/input"
import { cn } from "@workspace/ui/lib/utils"
import { t } from "@/i18n"

export interface SearchOption {
  value: string
  label: string
  /** 次要说明（所有者、状态等） */
  description?: string
  /** 参与搜索的附加关键词 */
  keywords?: string
  disabled?: boolean
  /** 不可选原因（展示在选项下方） */
  disabledReason?: string
  /** 选项右侧徽标 */
  badge?: ReactNode
}

interface SearchableSelectProps {
  options: SearchOption[]
  value: string | null
  onChange: (value: string) => void
  placeholder?: string
  searchPlaceholder?: string
  emptyText?: string
  /** 选项区最大高度（px），默认 320，超出内部滚动，避免下拉无限延伸 */
  listMaxHeight?: number
  disabled?: boolean
  className?: string
  /** 清除按钮（value 可空时显示） */
  clearable?: boolean
  onClear?: () => void
  /** 无障碍标签 */
  ariaLabel?: string
  /** 是否显示搜索框（短列表可关闭，键盘上下选择仍可用）；默认 true */
  searchable?: boolean
}

/**
 * 可搜索下拉选择器：
 * - 输入过滤 + 键盘上下选择 + Enter 确认 + Esc 关闭
 * - 列表限高滚动，不越出视口
 * - 不可选项置灰并说明原因
 */
export function SearchableSelect({
  options,
  value,
  onChange,
  placeholder,
  searchPlaceholder,
  emptyText,
  listMaxHeight = 320,
  disabled,
  className,
  clearable,
  onClear,
  ariaLabel,
  searchable = true,
}: SearchableSelectProps) {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState("")
  const [activeIndex, setActiveIndex] = useState(0)
  const rootRef = useRef<HTMLDivElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const listId = useId()

  const selected = useMemo(() => options.find((o) => o.value === value) ?? null, [options, value])

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return options
    return options.filter(
      (o) =>
        o.label.toLowerCase().includes(q) ||
        (o.description ?? "").toLowerCase().includes(q) ||
        (o.keywords ?? "").toLowerCase().includes(q) ||
        o.value.toLowerCase().includes(q),
    )
  }, [options, query])

  const openList = () => {
    if (disabled) return
    setQuery("")
    setActiveIndex(0)
    setOpen(true)
  }

  const commit = (opt: SearchOption) => {
    if (opt.disabled) return
    onChange(opt.value)
    setOpen(false)
  }

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!open) {
      if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
        e.preventDefault()
        openList()
      }
      return
    }
    if (e.key === "ArrowDown") {
      e.preventDefault()
      setActiveIndex((i) => Math.min(i + 1, filtered.length - 1))
    } else if (e.key === "ArrowUp") {
      e.preventDefault()
      setActiveIndex((i) => Math.max(i - 1, 0))
    } else if (e.key === "Enter") {
      e.preventDefault()
      const opt = filtered[activeIndex]
      if (opt) commit(opt)
    } else if (e.key === "Escape") {
      e.preventDefault()
      setOpen(false)
    }
  }

  // 激活项滚动到可视区（effect 中访问 ref，不在渲染期）
  useEffect(() => {
    if (!open || !listRef.current) return
    const el = listRef.current.querySelector(`[data-option-index="${activeIndex}"]`)
    el?.scrollIntoView({ block: "nearest" })
  }, [open, activeIndex])

  return (
    <div ref={rootRef} className={cn("relative", className)}>
      <div className="flex gap-2">
        <Button
          type="button"
          variant="outline"
          role="combobox"
          aria-expanded={open}
          aria-controls={listId}
          aria-label={ariaLabel}
          aria-haspopup="listbox"
          aria-activedescendant={open && !searchable && filtered[activeIndex] ? `${listId}-${activeIndex}` : undefined}
          disabled={disabled}
          onClick={() => (open ? setOpen(false) : openList())}
          onKeyDown={onKeyDown}
          className={cn(
            "h-10 w-full justify-between px-3 font-normal",
            !selected && "text-muted-foreground",
          )}
        >
          <span className="min-w-0 flex-1 truncate text-left">
            {selected ? (
              <span className="flex min-w-0 flex-col items-start">
                <span className="truncate text-sm">{selected.label}</span>
                {selected.description ? (
                  <span className="text-muted-foreground max-w-full truncate text-xs">{selected.description}</span>
                ) : null}
              </span>
            ) : (
              (placeholder ?? t.misc.choose)
            )}
          </span>
          <ChevronsUpDown className="text-muted-foreground size-4 shrink-0" aria-hidden />
        </Button>
        {clearable && value ? (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label={t.common.clear}
            className="text-muted-foreground size-10 shrink-0"
            onClick={() => {
              onClear?.()
              setOpen(false)
            }}
          >
            <X className="size-4" />
          </Button>
        ) : null}
      </div>

      {open ? (
        <>
          <div className="fixed inset-0 z-40" aria-hidden onClick={() => setOpen(false)} />
          <div
            className="bg-popover text-popover-foreground animate-in fade-in slide-in-from-top-1 absolute inset-x-0 top-full z-50 mt-1.5 overflow-hidden rounded-lg border shadow-lg duration-150"
            role="listbox"
            id={listId}
          >
            <div className="p-1">
              {searchable ? (
                <div className="border-b pb-2">
                  <div className="relative">
                    <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" aria-hidden />
                    <Input
                      autoFocus
                      value={query}
                      onChange={(e) => {
                        setQuery(e.target.value)
                        setActiveIndex(0)
                      }}
                      onKeyDown={onKeyDown}
                      placeholder={searchPlaceholder ?? t.common.search}
                      className="h-8 pl-8 text-sm"
                      aria-label={searchPlaceholder ?? t.common.search}
                      role="searchbox"
                    />
                  </div>
                </div>
              ) : null}
            </div>
            <div className="overflow-y-auto p-1" style={{ maxHeight: listMaxHeight }} ref={listRef}>
              {filtered.length === 0 ? (
                <p className="text-muted-foreground py-6 text-center text-xs">{emptyText ?? t.misc.noResults}</p>
              ) : (
                filtered.map((opt, i) => (
                  <button
                    key={opt.value}
                    type="button"
                    role="option"
                    id={`${listId}-${i}`}
                    aria-selected={opt.value === value}
                    aria-disabled={opt.disabled || undefined}
                    data-option-index={i}
                    onClick={() => commit(opt)}
                    onMouseEnter={() => setActiveIndex(i)}
                    className={cn(
                      "flex w-full flex-col gap-0.5 rounded-md px-2.5 py-2 text-left text-sm transition-colors",
                      i === activeIndex && "bg-accent",
                      opt.disabled && "text-muted-foreground cursor-not-allowed opacity-60",
                      opt.value === value && "font-medium",
                    )}
                  >
                    <span className="flex w-full items-center gap-2">
                      <span className="min-w-0 flex-1 truncate">{opt.label}</span>
                      {opt.badge}
                      {opt.value === value ? <Check className="text-primary size-3.5 shrink-0" aria-hidden /> : null}
                    </span>
                    {opt.description ? (
                      <span className="text-muted-foreground truncate text-xs">{opt.description}</span>
                    ) : null}
                    {opt.disabled && opt.disabledReason ? (
                      <span className="text-warning text-xs">{opt.disabledReason}</span>
                    ) : null}
                  </button>
                ))
              )}
            </div>
          </div>
        </>
      ) : null}
    </div>
  )
}
