import { Skeleton } from "@workspace/ui/components/skeleton"

/** 表格加载骨架：占位行 + 扫光（放在 TableBody 内使用） */
export function TableSkeleton({
  cols,
  rows = 5,
  widths,
}: {
  cols: number
  rows?: number
  /** 每列骨架宽度百分比（0-100），默认伪随机分布 */
  widths?: number[]
}) {
  const w = widths ?? Array.from({ length: cols }, (_, i) => 45 + ((i * 29) % 45))
  return (
    <>
      {Array.from({ length: rows }).map((_, r) => (
        <tr key={r} aria-hidden>
          {Array.from({ length: cols }).map((_, c) => (
            <td key={c} className="px-2 py-3">
              <Skeleton className="h-4" style={{ width: `${w[c % w.length]}%` }} />
            </td>
          ))}
        </tr>
      ))}
    </>
  )
}
