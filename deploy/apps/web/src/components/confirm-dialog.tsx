import { useState, type ReactNode } from "react"
import { Dialog } from "@workspace/ui/components/dialog"
import { Button } from "@workspace/ui/components/button"
import { Textarea } from "@workspace/ui/components/textarea"
import { Label } from "@workspace/ui/components/label"
import { t } from "@/i18n"

/** 危险操作二次确认；reasonLabel 存在时强制填写原因 */
export function ConfirmDialog({
  open,
  onClose,
  onConfirm,
  title,
  children,
  reasonLabel,
  confirmText = t.common.confirm,
  destructive = true,
}: {
  open: boolean
  onClose: () => void
  onConfirm: (reason: string) => void | Promise<void>
  title: ReactNode
  children?: ReactNode
  reasonLabel?: string
  confirmText?: string
  destructive?: boolean
}) {
  const [reason, setReason] = useState("")
  const [busy, setBusy] = useState(false)
  const ok = async () => {
    setBusy(true)
    try {
      await onConfirm(reason.trim())
      setReason("")
      onClose()
    } finally {
      setBusy(false)
    }
  }
  return (
    <Dialog
      open={open}
      onClose={() => {
        setReason("")
        onClose()
      }}
      title={title}
      footer={
        <>
          <Button variant="outline" onClick={onClose}>
            {t.common.cancel}
          </Button>
          <Button
            variant={destructive ? "destructive" : "default"}
            disabled={busy || (!!reasonLabel && reason.trim() === "")}
            onClick={ok}
          >
            {confirmText}
          </Button>
        </>
      }
    >
      <div className="space-y-3 text-sm">
        {children}
        {reasonLabel ? (
          <div className="space-y-1.5">
            <Label>{reasonLabel}{t.misc.reasonRequired}</Label>
            <Textarea value={reason} onChange={(e) => setReason(e.target.value)} rows={3} />
          </div>
        ) : null}
      </div>
    </Dialog>
  )
}
