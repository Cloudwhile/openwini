import { useId, useState } from "react"
import { Check, Loader2, Plus, X } from "lucide-react"
import { Button } from "@workspace/ui/components/button"
import { Input } from "@workspace/ui/components/input"
import { Textarea } from "@workspace/ui/components/textarea"
import { ApiError, api, apiErrMessage, type PermissionInfo } from "@/lib/api"
import { FormField } from "@/components/page-header"
import { SearchableSelect } from "@/components/searchable-select"
import { useToast } from "@/components/toast"
import { t } from "@/i18n"

export function PermissionRegister({
  known,
  onCreated,
}: {
  known: Record<string, PermissionInfo> | null
  onCreated: (permission: PermissionInfo) => void
}) {
  const id = useId()
  const { toast } = useToast()
  const [open, setOpen] = useState(false)
  const [name, setName] = useState("")
  const [risk, setRisk] = useState("high")
  const [description, setDescription] = useState("")
  const [busy, setBusy] = useState(false)
  const labels = t.admin.settings.perms
  const normalized = name.trim()
  const duplicate = !!known && Object.hasOwn(known, normalized)
  const valid = /^[a-z][a-z0-9_]*$/.test(normalized) && normalized.length <= 80

  async function save() {
    if (busy || !valid || duplicate) return
    setBusy(true)
    try {
      const permission = await api.post<PermissionInfo>(
        "/api/dev/permissions",
        { name: normalized, risk, description }
      )
      onCreated(permission)
      setOpen(false)
      setName("")
      setDescription("")
      setRisk("high")
      toast("success", labels.created)
    } catch (error) {
      toast(
        "error",
        error instanceof ApiError && error.status === 409
          ? labels.nameTaken
          : apiErrMessage(error)
      )
    } finally {
      setBusy(false)
    }
  }

  if (!open)
    return (
      <Button
        type="button"
        variant="ghost"
        size="icon"
        aria-label={labels.add}
        title={labels.add}
        onClick={() => setOpen(true)}
      >
        <Plus className="size-4" aria-hidden />
      </Button>
    )

  return (
    <div className="mt-3 space-y-3 border-t pt-3">
      <div className="grid gap-3 sm:grid-cols-2">
        <FormField
          label={labels.name}
          htmlFor={`${id}-name`}
          error={duplicate ? labels.nameTaken : undefined}
        >
          <Input
            id={`${id}-name`}
            value={name}
            maxLength={80}
            onChange={(event) => setName(event.target.value)}
            disabled={busy}
          />
        </FormField>
        <FormField label={labels.risk}>
          <SearchableSelect
            searchable={false}
            ariaLabel={labels.risk}
            value={risk}
            onChange={setRisk}
            options={["high", "medium", "low"].map((value) => ({
              value,
              label: t.permission.risk[value as "high" | "medium" | "low"],
            }))}
          />
        </FormField>
      </div>
      <FormField label={labels.description} htmlFor={`${id}-description`}>
        <Textarea
          id={`${id}-description`}
          value={description}
          maxLength={200}
          rows={2}
          onChange={(event) => setDescription(event.target.value)}
          disabled={busy}
        />
      </FormField>
      <div className="flex justify-end gap-2">
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={t.common.cancel}
          title={t.common.cancel}
          disabled={busy}
          onClick={() => setOpen(false)}
        >
          <X className="size-4" aria-hidden />
        </Button>
        <Button
          type="button"
          size="icon"
          aria-label={labels.add}
          title={labels.add}
          disabled={busy || !valid || duplicate}
          onClick={save}
        >
          {busy ? (
            <Loader2 className="size-4 animate-spin" aria-hidden />
          ) : (
            <Check className="size-4" aria-hidden />
          )}
        </Button>
      </div>
    </div>
  )
}
