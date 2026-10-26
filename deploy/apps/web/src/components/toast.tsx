import { createContext, useCallback, useContext, useState, type ReactNode } from "react"
import { CheckCircle2, XCircle } from "lucide-react"

type Toast = { id: number; kind: "success" | "error"; text: string }

const Ctx = createContext<{ toast: (kind: "success" | "error", text: string) => void }>({
  toast: () => {},
})

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<Toast[]>([])
  const toast = useCallback((kind: "success" | "error", text: string) => {
    const id = Date.now() + Math.random()
    setItems((xs) => [...xs, { id, kind, text }])
    setTimeout(() => setItems((xs) => xs.filter((x) => x.id !== id)), 4200)
  }, [])
  return (
    <Ctx.Provider value={{ toast }}>
      {children}
      <div className="fixed bottom-4 right-4 z-[60] flex w-80 flex-col gap-2">
        {items.map((t) => (
          <div
            key={t.id}
            className="bg-background flex items-start gap-2 rounded-lg border p-3 text-sm shadow-lg"
          >
            {t.kind === "success" ? (
              <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-emerald-500" />
            ) : (
              <XCircle className="mt-0.5 size-4 shrink-0 text-destructive" />
            )}
            <span className="min-w-0 break-words">{t.text}</span>
          </div>
        ))}
      </div>
    </Ctx.Provider>
  )
}

export function useToast() {
  return useContext(Ctx)
}
