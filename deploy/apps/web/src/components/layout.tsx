import { Link, NavLink, Outlet, useLocation, useNavigate } from "react-router-dom"
import { Code2, LayoutDashboard, LogOut } from "lucide-react"
import { useEffect, useState } from "react"
import { Button } from "@workspace/ui/components/button"
import { Badge } from "@workspace/ui/components/badge"
import { cn } from "@workspace/ui/lib/utils"
import { api, type SiteInfo } from "@/lib/api"
import { useAuth } from "@/lib/auth"
import { UserMenu } from "@/components/user-menu"
import { HeaderControls } from "@/components/header-controls"
import { getLang, t } from "@/i18n"

export function Layout() {
  const { me, refresh } = useAuth()
  const navigate = useNavigate()
  const location = useLocation()
  const [site, setSite] = useState<SiteInfo | null>(null)
  const dev = me?.developer
  const siteTitle = (getLang() === "en" && site?.siteNameEn ? site.siteNameEn : site?.siteName) ?? t.site.title

  useEffect(() => {
    api.get<SiteInfo>("/api/public/site").then(setSite).catch(() => {})
  }, [])

  const logout = async () => {
    await api.post("/api/auth/logout").catch(() => {})
    await refresh()
    navigate("/")
  }

  return (
    <div className="flex min-h-svh flex-col">
      <header className="bg-background/90 sticky top-0 z-40 border-b backdrop-blur">
        <div className="mx-auto flex h-14 max-w-6xl items-center gap-4 px-4 sm:px-6">
          <Link to="/" className="flex items-center gap-2.5 font-semibold">
            <img src="/logo.png" alt="WindowsIsland" className="size-7 rounded-md" />
            <span className="min-w-0 truncate">{siteTitle}</span>
          </Link>
          <nav className="hidden items-center gap-1 md:flex" aria-label={t.site.nav.catalog}>
            <NavLink
              to="/"
              end
              className={({ isActive }) =>
                cn(
                  "rounded-md px-3 py-1.5 text-sm transition-colors duration-150",
                  isActive
                    ? "bg-brand-muted text-primary font-medium"
                    : "text-muted-foreground hover:bg-muted hover:text-foreground",
                )
              }
            >
              {t.site.nav.catalog}
            </NavLink>
          </nav>
          <div className="ml-auto flex items-center gap-1.5">
            <HeaderControls />
            {dev ? (
              <>
                {dev.isBanned ? (
                  <Badge variant="destructive" className="hidden sm:inline-flex">
                    {t.misc.banned}
                  </Badge>
                ) : null}
                <Button
                  size="sm"
                  variant="outline"
                  className="gap-1.5"
                  onClick={() => navigate(dev.isAdmin ? "/admin" : "/me")}
                >
                  <LayoutDashboard className="size-3.5" />
                  {t.misc.console}
                </Button>
                {/* 公开页头使用精简用户菜单（含角色切换与退出） */}
                <UserMenu dev={dev} onLogout={logout} />
                <Button
                  variant="ghost"
                  size="icon"
                  className="text-muted-foreground hidden sm:inline-flex"
                  title={t.site.nav.logout}
                  aria-label={t.site.nav.logout}
                  onClick={logout}
                >
                  <LogOut className="size-4" />
                </Button>
              </>
            ) : (
              <Button variant="outline" size="sm" className="gap-1.5" onClick={() => navigate("/login")}>
                <Code2 className="size-4" />
                {t.site.nav.login}
              </Button>
            )}
          </div>
        </div>
        {site?.announcement ? (
          <div className="bg-muted text-muted-foreground border-t px-4 py-1.5 text-center text-xs" role="note">
            {site.announcement}
          </div>
        ) : null}
      </header>
      <main className="flex-1">
        <div key={location.pathname} className="animate-in mx-auto max-w-6xl px-4 py-10 fade-in duration-300 sm:px-6">
          <Outlet />
        </div>
      </main>
      <footer className="bg-muted/30 border-t">
        <div className="text-muted-foreground mx-auto flex max-w-6xl flex-col gap-2 px-4 py-8 text-xs leading-relaxed sm:flex-row sm:items-center sm:justify-between sm:px-6">
          <div className="flex items-center gap-2">
            <img src="/logo.png" alt="" className="size-5 rounded-sm opacity-80" />
            <span>{siteTitle}</span>
          </div>
          <span className="max-w-xl sm:text-right">{t.site.footer}</span>
        </div>
      </footer>
    </div>
  )
}
