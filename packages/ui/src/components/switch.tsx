import type { ComponentProps } from "react"
import { cn } from "cn"

type SwitchProps = Omit<ComponentProps<"button">, "onChange"> & {
  checked: boolean
  onCheckedChange: (checked: boolean) => void
}

function Switch({ className, checked, onCheckedChange, disabled, ...props }: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      data-state={checked ? "checked" : "unchecked"}
      disabled={disabled}
      data-slot="switch"
      className={cn(
        "peer inline-flex h-5 w-9 shrink-0 items-center rounded-full border border-transparent shadow-xs outline-none transition-all disabled:cursor-not-allowed disabled:opacity-50",
        checked ? "bg-primary" : "bg-input",
        className,
      )}
      onClick={() => onCheckedChange(!checked)}
      {...props}
    >
      <span
        className={cn(
          "bg-background pointer-events-none block size-4 rounded-full ring-0 shadow transition-transform",
          checked ? "translate-x-[18px]" : "translate-x-0.5",
        )}
      />
    </button>
  )
}

export { Switch }
