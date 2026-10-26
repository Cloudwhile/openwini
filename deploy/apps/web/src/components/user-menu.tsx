import { useEffect, useRef, useState } from "react"
import { useNavigate } from "react-router-dom"
import { LogOut, ShieldCheck, Wrench } from "lucide-react"
import { cn } from "cn"
import { t } from "@/i18n"
import { api, type Me } from "@/lib/api"
import { useToast } from "@/components/toast"

/** 顶栏用户圆形头像：有图用图，无图用首字母 */
export function Avatar({ src, name }: { src?: string | null; name: string }) {
  if (src) {
    return <img src={src} alt={name} className="size-8 rounded-full border object-cover" />
  }
  const letter = (name || "?").slice(0, 1).toUpperCase()
  return (
    <span className="bg-primary/10 text-primary flex size-8 items-center justify-center rounded-full border text-xs font-semibold">
      {letter}
    </span>
  )
}

/**
 * 点击头像的下拉菜单：切换管理后台 / 个人后台、退出登录。
 * 点击外部或 Esc 关闭。
 */
export function UserMenu({
  dev,
  onLogout,
}: {
  dev: NonNullable<Me["developer"]>
  onLogout: () => void

}) {
  const [open, setOpen] = useState(false)
  const [savingLanguage, setSavingLanguage] = useState(false)
  const { toast } = useToast()
  const ref = useRef<HTMLDivElement>(null)
  const navigate = useNavigate()

  useEffect(() => {
    if (!open) return
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false)
    }
    document.addEventListener("mousedown", onClick)
    document.addEventListener("keydown", onKey)
    return () => {
      document.removeEventListener("mousedown", onClick)
      document.removeEventListener("keydown", onKey)
    }
  }, [open])

  const go = (path: string) => {
    setOpen(false)
    navigate(path)
  }

  const saveLanguage = async (value: string) => {
    if (value !== "zh" && value !== "en" || savingLanguage) return
    setSavingLanguage(true)
    try {
      await api.put("/api/dev/profile", {
        displayName: dev.displayName || dev.developerId,
        preferredLanguage: value,
      })
      window.location.reload()
    } catch {
      setSavingLanguage(false)
      toast("error", t.misc.emailLanguageSaveFailed)
    }
  }

  const items: { label: string; icon: React.ReactNode; path: string; current: boolean }[] = [
    {
      label: t.misc.adminArea,
      icon: <ShieldCheck className="size-4" />,
      path: "/admin",
      current: window.location.pathname.startsWith("/admin"),
    },
    {
      label: t.misc.personalConsole,
      icon: <Wrench className="size-4" />,
      path: "/me",
      current: window.location.pathname.startsWith("/me"),
    },
  ]

  return (
    <div ref={ref} className="relative">
      {/* ID + 头像整体为点击热区 */}
      <button
        className="flex items-center gap-2 rounded-full pr-1 pl-1 outline-none transition-colors hover:bg-muted/60 focus-visible:ring-ring/50 focus-visible:ring-[3px]"
        onClick={() => setOpen((v) => !v)}
        title={dev.developerId}
      >
        <span className="text-muted-foreground hidden max-w-32 truncate font-mono text-xs sm:inline">
          {dev.developerId}
        </span>
        <Avatar src={dev.avatarUrl} name={dev.displayName || dev.developerId} />
      </button>
      {open ? (
        <div
          className={cn(
            "bg-background animate-in fade-in zoom-in-95 absolute top-full right-0 z-50 mt-2 w-60 overflow-hidden rounded-xl border p-1 shadow-lg",
            "duration-150",
          )}
        >
          <div className="px-3 py-2">
            <div className="truncate text-sm font-medium">{dev.displayName || dev.developerId}</div>
            <div className="text-muted-foreground truncate font-mono text-xs">{dev.developerId}</div>
          </div>
          <label className="text-muted-foreground flex items-center justify-between gap-3 px-3 py-2 text-xs">
            <span>{t.misc.emailLanguage}</span>
            <select
              aria-label={t.misc.emailLanguage}
              className="bg-background text-foreground rounded-md border px-2 py-1 text-xs"
              value={dev.preferredLanguage ?? "zh"}
              disabled={savingLanguage}
              onChange={(e) => void saveLanguage(e.target.value)}
            >
              <option value="zh">{t.misc.emailLanguageZh}</option>
              <option value="en">{t.misc.emailLanguageEn}</option>
            </select>
          </label>
          <div className="my-1 border-t" />
          {items
            .filter((i) => i.path !== "/admin" || dev.isAdmin)
            .map((i) => (
              <button
                key={i.path}
                onClick={() => go(i.path)}
                className={cn(
                  "flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-sm transition-colors",
                  i.current
                    ? "bg-primary/10 text-primary font-medium"
                    : "text-muted-foreground hover:bg-muted hover:text-foreground",
                )}
              >
                {i.icon}
                {i.label}
                {i.current ? (
                  <span className="text-primary ml-auto text-xs">{t.misc.currentHere}</span>
                ) : null}
              </button>
            ))}
          <div className="my-1 border-t" />
          <button
            onClick={() => {
              setOpen(false)
              onLogout()
            }}
            className="text-destructive hover:bg-destructive/10 flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-sm transition-colors"
          >
            <LogOut className="size-4" />
            {t.site.nav.logout}
          </button>
        </div>
      ) : null}
    </div>
  )
}
