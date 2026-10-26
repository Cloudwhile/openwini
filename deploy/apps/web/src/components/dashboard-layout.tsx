import { NavLink, Outlet, useNavigate } from "react-router-dom"
import {
  ArrowLeft,
  Blocks,
  ChevronsLeft,
  FileClock,
  Menu,
  ScrollText,
  Settings,
  ShieldCheck,
  Users,
  Wrench,
  X,
} from "lucide-react"
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react"
import { Button } from "@workspace/ui/components/button"
import { cn } from "@workspace/ui/lib/utils"
import { api, type SiteInfo } from "@/lib/api"
import { useAuth } from "@/lib/auth"
import { UserMenu } from "@/components/user-menu"
import { HeaderControls } from "@/components/header-controls"
import { getLang, t } from "@/i18n"

/** 后台侧边导航项：激活态用品牌色底 + 左侧指示，唯一高亮 */
function SideNavItem({
  to,
  icon,
  label,
  end,
  onNavigate,
}: {
  to: string
  icon: ReactNode
  label: string
  end?: boolean
  onNavigate?: () => void
}) {
  return (
    <NavLink
      to={to}
      end={end}
      onClick={onNavigate}
      className={({ isActive }) =>
        cn(
          "relative flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm transition-colors duration-150",
          isActive
            ? "bg-brand-muted text-primary font-medium"
            : "text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
        )
      }
    >
      {({ isActive }) => (
        <>
          {isActive ? (
            <span className="bg-primary absolute top-1/2 left-0 h-4 w-0.5 -translate-y-1/2 rounded-full" aria-hidden />
          ) : null}
          <span className="shrink-0 [&>svg]:size-4">{icon}</span>
          <span className="truncate">{label}</span>
        </>
      )}
    </NavLink>
  )
}

function useAreaMenu(area: "developer" | "admin", onNavigate?: () => void) {
  if (area === "admin") {
    return (
      <>
        <SideNavItem to="/admin" end icon={<Blocks />} label={t.misc.overview} onNavigate={onNavigate} />
        <SideNavItem to="/admin/review" icon={<ShieldCheck />} label={t.site.nav.adminReview} onNavigate={onNavigate} />
        <SideNavItem to="/admin/plugins" icon={<Wrench />} label={t.site.nav.adminPlugins} onNavigate={onNavigate} />
        <SideNavItem to="/admin/developers" icon={<Users />} label={t.site.nav.adminDevelopers} onNavigate={onNavigate} />
        <SideNavItem to="/admin/settings" icon={<Settings />} label={t.site.nav.adminSettings} onNavigate={onNavigate} />
        <SideNavItem to="/admin/audit" icon={<FileClock />} label={t.site.nav.adminAudit} onNavigate={onNavigate} />
      </>
    )
  }
  return (
    <>
      <SideNavItem to="/me" end icon={<Blocks />} label={t.misc.overview} onNavigate={onNavigate} />
      <SideNavItem to="/me/plugins" icon={<Wrench />} label={t.site.nav.myPlugins} onNavigate={onNavigate} />
      <SideNavItem to="/me/plugins/new" icon={<ScrollText />} label={t.misc.submitPluginNav} onNavigate={onNavigate} />
    </>
  )
}

/** 侧边栏宽度（可拖拽调整，双击分隔条复位；拖到阈值以下收起为纯图标态） */
const SIDEBAR_DEFAULT_W = 240
const SIDEBAR_MIN_W = 200
const SIDEBAR_MAX_W = 320
const SIDEBAR_COLLAPSED_W = 64
const SIDEBAR_COLLAPSE_BELOW = 150

function BrandButton({ siteTitle, onClick }: { siteTitle: string; onClick: () => void }) {
  return (
    <button
      className="flex h-14 w-full items-center gap-2.5 px-4 text-left transition-colors hover:bg-muted/50"
      onClick={onClick}
      title={t.misc.backToCatalog}
    >
      <img src="/logo.png" alt="" className="size-7 rounded-md" />
      <span className="min-w-0 flex-1 truncate text-sm font-semibold">{siteTitle}</span>
    </button>
  )
}

/**
 * 后台统一布局：顶部 Header(56px) + 左侧 Sidebar(240px，可折叠) + 右侧 Content。
 * 窄屏（<lg）侧边栏变为抽屉。角色切换统一在 Header 右侧用户菜单完成。
 */
export function DashboardLayout({ area }: { area: "developer" | "admin" }) {
  const { me, refresh } = useAuth()
  const navigate = useNavigate()
  const [site, setSite] = useState<SiteInfo | null>(null)
  const [collapsed, setCollapsed] = useState(false)
  const [mobileOpen, setMobileOpen] = useState(false)
  // 侧边栏宽度：持久化；宽度完全由 DOM 直写（React 不管理 style.width），
  // 拖拽期间零重渲染——每帧只做一次 style 写入，杜绝快速拖动的卡顿
  const [width, setWidth] = useState(() => {
    const v = Number(localStorage.getItem("sidebar_width"))
    return Number.isFinite(v) && v >= SIDEBAR_MIN_W && v <= SIDEBAR_MAX_W ? v : SIDEBAR_DEFAULT_W
  })
  const [dragging, setDragging] = useState(false)
  // 拖拽中是否低于图标阈值（仅在跨越阈值时 setState，一两次渲染）
  const [dragNarrow, setDragNarrow] = useState(false)
  const asideRef = useRef<HTMLElement | null>(null)
  const showLabels = dragging ? !dragNarrow : !collapsed
  const effectiveWidth = collapsed ? SIDEBAR_COLLAPSED_W : width

  useLayoutEffect(() => {
    if (asideRef.current) asideRef.current.style.width = `${effectiveWidth}px`
  }, [effectiveWidth])
  const dev = me?.developer
  const lang = getLang()
  const siteTitle = lang === "en" && site?.siteNameEn ? site.siteNameEn : (site?.siteName ?? t.site.title)
  const areaBadge = area === "admin" ? t.misc.adminArea : t.misc.developerArea

  useEffect(() => {
    api.get<SiteInfo>("/api/public/site").then(setSite).catch(() => {})
  }, [])

  // 会话缺失/过期：自动跳登录页并携带回跳地址（与 API 层 401 拦截、路由守卫互为兜底）
  useEffect(() => {
    if (me && !me.authenticated) {
      const current = window.location.pathname + window.location.search
      window.location.href = `/login?error=${encodeURIComponent(t.misc.sessionExpired)}&redirect=${encodeURIComponent(current)}`
    }
  }, [me, lang])

  // 路由切换时由导航项的 onClick 收起移动端抽屉（避免在 effect 中同步 setState）

  const logout = async () => {
    await api.post("/api/auth/logout").catch(() => {})
    await refresh()
    navigate("/")
  }

  useEffect(() => {
    if (width !== SIDEBAR_DEFAULT_W) localStorage.setItem("sidebar_width", String(width))
    else localStorage.removeItem("sidebar_width")
  }, [width])

  /** 拖拽调整侧边栏宽度：宽度直写 DOM（每帧一次 style 写入，无 React 渲染）；
   *  低于阈值时内容切换为图标态；松手按最终位置吸附（带过渡动画）；双击分隔条复位 */
  const startResize = (e: React.MouseEvent) => {
    e.preventDefault()
    const startX = e.clientX
    const startW = effectiveWidth
    let lastRaw = startW
    setDragging(true)
    document.body.style.userSelect = "none"
    document.body.style.cursor = "col-resize"
    const onMove = (ev: MouseEvent) => {
      lastRaw = Math.min(SIDEBAR_MAX_W, Math.max(SIDEBAR_COLLAPSED_W, startW + (ev.clientX - startX)))
      if (asideRef.current) asideRef.current.style.width = `${lastRaw}px`
      setDragNarrow(lastRaw < SIDEBAR_COLLAPSE_BELOW)
    }
    const onUp = () => {
      document.removeEventListener("mousemove", onMove)
      document.removeEventListener("mouseup", onUp)
      document.body.style.userSelect = ""
      document.body.style.cursor = ""
      setDragging(false)
      setDragNarrow(false)
      if (lastRaw < SIDEBAR_COLLAPSE_BELOW) {
        setCollapsed(true)
      } else {
        setCollapsed(false)
        setWidth(Math.min(SIDEBAR_MAX_W, Math.max(SIDEBAR_MIN_W, lastRaw)))
      }
    }
    document.addEventListener("mousemove", onMove)
    document.addEventListener("mouseup", onUp)
  }

  const menu = useAreaMenu(area)
  const mobileMenu = useAreaMenu(area, () => setMobileOpen(false))

  return (
    <div className="bg-muted/30 flex min-h-svh">
      {/* 桌面侧边栏（可折叠） */}
      {/* 桌面侧边栏（可折叠 + 可拖拽调宽） */}
      <aside
        ref={asideRef}
        className={cn(
          "bg-sidebar relative sticky top-0 hidden h-svh shrink-0 flex-col border-r lg:flex",
          !dragging && "transition-[width] duration-200",
        )}
      >
        {!showLabels ? (
          <button className="flex h-14 w-full items-center justify-center" onClick={() => navigate("/")} title={t.misc.backToCatalog}>
            <img src="/logo.png" alt="" className="size-7 rounded-md" />
          </button>
        ) : (
          <BrandButton siteTitle={siteTitle} onClick={() => navigate("/")} />
        )}
        <nav className={cn("flex-1 space-y-1 overflow-y-auto p-2", !showLabels && "px-2")} aria-label={areaBadge}>
          {!showLabels ? (
            <>
              {/* 折叠态：仅图标 */}
              {(area === "admin"
                ? [
                    { to: "/admin", end: true, icon: <Blocks />, label: t.misc.overview },
                    { to: "/admin/review", icon: <ShieldCheck />, label: t.site.nav.adminReview },
                    { to: "/admin/plugins", icon: <Wrench />, label: t.site.nav.adminPlugins },
                    { to: "/admin/developers", icon: <Users />, label: t.site.nav.adminDevelopers },
                    { to: "/admin/settings", icon: <Settings />, label: t.site.nav.adminSettings },
                    { to: "/admin/audit", icon: <FileClock />, label: t.site.nav.adminAudit },
                  ]
                : [
                    { to: "/me", end: true, icon: <Blocks />, label: t.misc.overview },
                    { to: "/me/plugins", icon: <Wrench />, label: t.site.nav.myPlugins },
                    { to: "/me/plugins/new", icon: <ScrollText />, label: t.misc.submitPluginNav },
                  ]
              ).map((item) => (
                <SideNavItem key={item.to} to={item.to} end={item.end} icon={item.icon} label={item.label} />
              ))}
            </>
          ) : (
            menu
          )}
        </nav>
        <div className="border-t p-2">
          {!showLabels ? (
            <Button variant="ghost" size="icon" className="text-muted-foreground w-full" onClick={() => navigate("/")} title={t.misc.backToCatalog}>
              <ArrowLeft className="size-4" />
            </Button>
          ) : (
            <Button
              variant="ghost"
              size="sm"
              className="text-muted-foreground w-full justify-start gap-2.5"
              onClick={() => navigate("/")}
            >
              <ArrowLeft className="size-4" />
              {t.misc.backToCatalog}
            </Button>
          )}
        </div>
        {/* 拖拽调宽分隔条（z 低于折叠开关，双击复位） */}
        <div
          role="separator"
          aria-orientation="vertical"
          aria-label={t.misc.resizeSidebar}
          title={t.misc.resizeSidebar}
          className="hover:bg-primary/30 absolute inset-y-0 -right-1 z-0 w-2 cursor-col-resize transition-colors"
          onMouseDown={startResize}
          onDoubleClick={() => setWidth(SIDEBAR_DEFAULT_W)}
        />
        {/* 折叠开关 */}
        <button
          className="text-muted-foreground hover:bg-muted hover:text-foreground absolute top-16 -right-3 z-10 flex size-6 items-center justify-center rounded-full border bg-card shadow-sm transition-colors"
          onClick={() => setCollapsed((v) => !v)}
          aria-label={collapsed ? t.misc.expandSidebar : t.misc.collapseSidebar}
          aria-expanded={!collapsed}
        >
          <ChevronsLeft className={cn("size-3.5 transition-transform duration-200", !showLabels && "rotate-180")} />
        </button>
      </aside>

      {/* 移动端抽屉 */}
      {mobileOpen ? (
        <div className="fixed inset-0 z-50 lg:hidden">
          {/* 遮罩用黑色半透明（与 Dialog 一致）：暗色主题下压暗而非提亮 */}
          <div className="bg-black/50 animate-in fade-in absolute inset-0" onClick={() => setMobileOpen(false)} aria-hidden />
          <aside className="bg-sidebar animate-in slide-in-from-left absolute inset-y-0 left-0 flex w-64 flex-col border-r shadow-xl duration-200">
            <div className="flex items-center justify-between pr-2">
              <BrandButton siteTitle={siteTitle} onClick={() => navigate("/")} />
              <Button variant="ghost" size="icon" aria-label={t.common.close} onClick={() => setMobileOpen(false)}>
                <X className="size-4" />
              </Button>
            </div>
            <nav className="flex-1 space-y-1 overflow-y-auto p-3" aria-label={areaBadge}>
              {mobileMenu}
            </nav>
            <div className="border-t p-3">
              <Button variant="ghost" size="sm" className="text-muted-foreground w-full justify-start gap-2.5" onClick={() => navigate("/")}>
                <ArrowLeft className="size-4" />
                {t.misc.backToCatalog}
              </Button>
            </div>
          </aside>
        </div>
      ) : null}

      {/* 右侧：Header + 内容 */}
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="bg-background/90 sticky top-0 z-40 flex h-14 items-center gap-3 border-b px-4 backdrop-blur lg:px-8">
          <Button variant="ghost" size="icon" className="lg:hidden" aria-label={t.misc.openMenu} onClick={() => setMobileOpen(true)}>
            <Menu className="size-5" />
          </Button>
          <span className="text-muted-foreground text-sm font-medium">{areaBadge}</span>
          <div className="ml-auto flex items-center gap-1">
            <HeaderControls />
            {dev ? <UserMenu dev={dev} onLogout={logout} /> : null}
          </div>
        </header>

        <main className="min-w-0 flex-1 px-4 py-6 sm:px-6 lg:px-8">
          <Outlet />
        </main>
      </div>
    </div>
  )
}
