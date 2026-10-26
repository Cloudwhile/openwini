import { Button } from "@workspace/ui/components/button"
import { t } from "@/i18n"

export function Pagination({
  page,
  pageSize,
  total,
  onPage,
}: {
  page: number
  pageSize: number
  total: number
  onPage: (p: number) => void
}) {
  const pages = Math.max(1, Math.ceil(total / pageSize))
  if (pages <= 1) return null
  return (
    <div className="flex items-center justify-between gap-2 pt-2">
      <span className="text-muted-foreground text-xs">{t.common.total(total)}</span>
      <div className="flex items-center gap-2">
        <Button variant="outline" size="sm" disabled={page <= 1} onClick={() => onPage(page - 1)}>
          {t.common.prev}
        </Button>
        <span className="text-muted-foreground text-sm">
          {page} / {pages}
        </span>
        <Button variant="outline" size="sm" disabled={page >= pages} onClick={() => onPage(page + 1)}>
          {t.common.next}
        </Button>
      </div>
    </div>
  )
}
