import { useEffect, useState } from "react"
import { useNavigate, useSearchParams } from "react-router-dom"
import { Code2, ShieldCheck, Zap } from "lucide-react"
import { Button } from "@workspace/ui/components/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@workspace/ui/components/card"
import { api, type SiteInfo } from "@/lib/api"
import { useAuth } from "@/lib/auth"
import { clearLoginRedirect, loginRedirect, rememberLoginRedirect } from "@/lib/login-redirect"
import { HeaderControls } from "@/components/header-controls"
import { getLang, t } from "@/i18n"

/** 登录页：品牌化分栏设计（左侧特性介绍，右侧 GitHub 登录卡） */
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

  // 记录登录前的目标页（会话过期跳转携带 redirect），登录回站后由仪表盘续跳
  useEffect(() => {
    if (redirect) rememberLoginRedirect(redirect)
    else if (!error) clearLoginRedirect()
  }, [redirect, error])

  // 已登录访问登录页：直接进入对应控制台
  useEffect(() => {
    if (me?.authenticated) {
      navigate(redirect ?? (me.developer?.isAdmin ? "/admin" : "/me"), { replace: true })
    }
  }, [me, redirect, navigate])

  const features = [
    { icon: <ShieldCheck className="size-5" aria-hidden />, title: t.login.hero.f1, desc: t.login.hero.f1d },
    { icon: <Zap className="size-5" aria-hidden />, title: t.login.hero.f2, desc: t.login.hero.f2d },
    { icon: <Code2 className="size-5" aria-hidden />, title: t.login.hero.f3, desc: t.login.hero.f3d },
  ]

  return (
    <div className="from-background via-background to-muted/50 grid min-h-svh bg-gradient-to-br lg:grid-cols-2">
      {/* 品牌区 */}
      <div className="relative hidden flex-col justify-between overflow-hidden p-12 lg:flex">
        <div className="pointer-events-none absolute -top-32 -left-32 size-96 rounded-full bg-primary/10 blur-3xl" aria-hidden />
        <div className="pointer-events-none absolute -right-24 bottom-0 size-80 rounded-full bg-primary/5 blur-3xl" aria-hidden />
        <button className="relative flex items-center gap-2.5 text-lg font-semibold" onClick={() => navigate("/")}>
          <img src="/logo.png" alt="" className="size-8 rounded-md" />
          {siteTitle}
        </button>
        <div className="relative space-y-8">
          <h1 className="max-w-md text-4xl leading-tight font-bold tracking-tight">
            {t.login.hero.titleA}
            <span className="text-primary">{t.login.hero.titleB}</span>
          </h1>
          <div className="space-y-5">
            {features.map((f) => (
              <div key={f.title} className="flex items-start gap-3.5">
                <div className="bg-primary/10 text-primary mt-0.5 flex size-10 shrink-0 items-center justify-center rounded-lg">
                  {f.icon}
                </div>
                <div>
                  <div className="text-sm font-medium">{f.title}</div>
                  <div className="text-muted-foreground text-sm">{f.desc}</div>
                </div>
              </div>
            ))}
          </div>
        </div>
        <div className="text-muted-foreground relative text-xs">{t.site.footer}</div>
      </div>

      {/* 登录卡 */}
      <div className="flex items-center justify-center p-6">
        <div className="w-full max-w-md space-y-4">
          <div className="flex items-center justify-between lg:hidden">
            <button className="flex items-center gap-2 font-semibold" onClick={() => navigate("/")}>
              <img src="/logo.png" alt="" className="size-7 rounded-md" />
              {siteTitle}
            </button>
          </div>
          <div className="flex justify-end gap-1">
            <HeaderControls />
          </div>
          <Card className="shadow-lg">
            <CardHeader className="gap-2">
              <CardTitle className="text-xl">{t.login.title}</CardTitle>
              <CardDescription className="leading-relaxed">{t.login.description}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {error ? (
                <div className="border-destructive/40 bg-destructive/5 rounded-lg border p-3 text-sm text-destructive">
                  {t.login.oauthFailed}：{error}
                </div>
              ) : null}
              <Button className="h-11 w-full text-base" onClick={() => (window.location.href = "/api/auth/github/start")}>
                <Code2 className="size-5" aria-hidden />
                {t.login.button}
              </Button>
              <p className="text-muted-foreground text-center text-xs">{t.login.hero.consent}</p>
            </CardContent>
          </Card>
          <div className="text-center">
            <button className="text-muted-foreground hover:text-foreground text-xs" onClick={() => navigate("/")}>
              ← {t.misc.backToCatalog}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
