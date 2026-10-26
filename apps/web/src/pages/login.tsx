import { useEffect, useState } from "react"
import { Link, useNavigate, useSearchParams } from "react-router-dom"
import { AlertCircle, ArrowLeft, Code2 } from "lucide-react"
import { api, type SiteInfo } from "@/lib/api"
import { useAuth } from "@/lib/auth"
import { clearLoginRedirect, loginRedirect, rememberLoginRedirect } from "@/lib/login-redirect"
import { HeaderControls } from "@/components/header-controls"
import { getLang, t } from "@/i18n"

export function Login() {
  const [params] = useSearchParams()
  const navigate = useNavigate()
  const { me } = useAuth()
  const error = params.get("error")
  const redirect = loginRedirect(params.get("redirect"))
  const [site, setSite] = useState<SiteInfo | null>(null)
  const siteTitle = getLang() === "en" && site?.siteNameEn ? site.siteNameEn : (site?.siteName ?? t.site.title)

  useEffect(() => {
    api.get<SiteInfo>("/api/public/site").then(setSite).catch(() => {})
  }, [])

  useEffect(() => {
    if (redirect) rememberLoginRedirect(redirect)
    else if (!error) clearLoginRedirect()
  }, [redirect, error])

  useEffect(() => {
    if (me?.authenticated) {
      navigate(redirect ?? (me.developer?.isAdmin ? "/admin" : "/me"), { replace: true })
    }
  }, [me, redirect, navigate])

  return (
    <div className="flex min-h-svh flex-col bg-background">
      <header className="flex items-center justify-between gap-4 border-b px-5 py-4">
        <Link to="/" className="flex min-w-0 items-center gap-2 font-semibold">
          <img src="/logo.png" alt="" className="size-7 rounded-md" />
          <span className="truncate">{siteTitle}</span>
        </Link>
        <div className="flex shrink-0 items-center gap-1"><HeaderControls /></div>
      </header>
      <main className="mx-auto flex w-full max-w-md flex-1 flex-col justify-center gap-6 px-6 py-12">
        <h1 className="text-2xl font-semibold tracking-tight">{t.login.title}</h1>
        {error ? (
          <div role="alert" className="flex items-start gap-2 border-l-2 border-destructive pl-3 text-sm text-destructive">
            <AlertCircle className="mt-0.5 size-4 shrink-0" aria-hidden />
            <span>{t.login.oauthFailed}：{error}</span>
          </div>
        ) : null}
        <a href="/api/auth/github/start" className="inline-flex w-fit items-center gap-2 border-b border-primary py-3 font-medium text-primary hover:opacity-80">
          <Code2 className="size-5" aria-hidden />
          {t.login.button}
        </a>
        <Link to="/" className="inline-flex w-fit items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground">
          <ArrowLeft className="size-4" aria-hidden />
          {t.login.backHome}
        </Link>
      </main>
    </div>
  )
}
