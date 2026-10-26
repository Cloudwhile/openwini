import { BrowserRouter, Navigate, Route, Routes, useLocation, useNavigate, useParams } from "react-router-dom"
import { lazy, Suspense, useEffect } from "react"
import { useLang } from "@/i18n"
import { AuthProvider, useRequireAdmin, useRequireAuth } from "@/lib/auth"
import { ToastProvider } from "@/components/toast"
import { ForbiddenState, LoadingState } from "@/components/page-states"
import { Layout } from "@/components/layout"
import { DashboardLayout } from "@/components/dashboard-layout"
import { ErrorBoundary } from "@/components/error-boundary"
import { Home } from "@/pages/home"

const PluginDetailPage = lazy(() => import("@/pages/plugin-detail").then((m) => ({ default: m.PluginDetailPage })))
const DeveloperPage = lazy(() => import("@/pages/developer").then((m) => ({ default: m.DeveloperPage })))
const Login = lazy(() => import("@/pages/login").then((m) => ({ default: m.Login })))
const DeveloperDashboard = lazy(() => import("@/pages/dev/dashboard").then((m) => ({ default: m.DeveloperDashboard })))
const MyPlugins = lazy(() => import("@/pages/dev/my-plugins").then((m) => ({ default: m.MyPlugins })))
const PluginWorkspace = lazy(() => import("@/pages/dev/plugin-workspace").then((m) => ({ default: m.PluginWorkspace })))
const SubmitPage = lazy(() => import("@/pages/dev/submit").then((m) => ({ default: m.SubmitPage })))
const AdminDashboard = lazy(() => import("@/pages/admin/dashboard").then((m) => ({ default: m.AdminDashboard })))
const ReviewQueue = lazy(() => import("@/pages/admin/review-queue").then((m) => ({ default: m.ReviewQueue })))
const AdminPlugins = lazy(() => import("@/pages/admin/plugins").then((m) => ({ default: m.AdminPlugins })))
const AdminDevelopers = lazy(() => import("@/pages/admin/developers").then((m) => ({ default: m.AdminDevelopers })))
const AdminSettings = lazy(() => import("@/pages/admin/settings").then((m) => ({ default: m.AdminSettings })))
const AdminAudit = lazy(() => import("@/pages/admin/audit").then((m) => ({ default: m.AdminAudit })))

function SubmitRoute({ mode }: { mode: "new" | "edit" | "newVersion" | "official" | "officialVersion" }) {
  const { pluginId } = useParams()
  return <SubmitPage key={`${mode}:${pluginId ?? ""}`} mode={mode} />
}

function WorkspaceRoute() {
  const { pluginId } = useParams()
  return <PluginWorkspace key={pluginId} />
}

/** 路由级登录守卫：未登录跳转 /login 并携带回跳地址 */
function RequireAuth({ children }: { children: React.ReactNode }) {
  const { ready, logged } = useRequireAuth()
  const navigate = useNavigate()
  const location = useLocation()
  useEffect(() => {
    if (ready && !logged) {
      const target = location.pathname + location.search
      navigate(`/login?redirect=${encodeURIComponent(target)}`, { replace: true })
    }
  }, [ready, logged, navigate, location])
  if (!ready || !logged) return <LoadingState />
  return <>{children}</>
}

/** 路由级管理员守卫：未登录跳转登录页；已登录但非管理员展示明确禁止态（服务端仍有最终鉴权） */
function RequireAdmin({ children }: { children: React.ReactNode }) {
  const { ready, logged, isAdmin } = useRequireAdmin()
  const navigate = useNavigate()
  const location = useLocation()
  useEffect(() => {
    if (ready && !logged) {
      const target = location.pathname + location.search
      navigate(`/login?redirect=${encodeURIComponent(target)}`, { replace: true })
    }
  }, [ready, logged, navigate, location])
  if (!ready || !logged) return <LoadingState />
  if (!isAdmin) {
    return (
      <div className="px-6 py-10">
        <ForbiddenState />
      </div>
    )
  }
  return <>{children}</>
}

export function App() {
  // 语言切换时以 key 重挂载整棵路由树，让模块级 t 代理按新语言重新解析
  const lang = useLang()
  return (
    <ErrorBoundary key={lang}>
      <BrowserRouter>
      <AuthProvider>
        <ToastProvider>
          <Suspense fallback={<LoadingState />}>
            <Routes>
            {/* 前台门户（顶部导航） */}
            <Route element={<Layout />}>
              <Route index element={<Home />} />
              <Route path="plugins/:pluginId" element={<PluginDetailPage />} />
              <Route path="developers/:developerId" element={<DeveloperPage />} />
            </Route>
            {/* 登录页（独立品牌化布局） */}
            <Route path="login" element={<Login />} />
            {/* 开发者后台（侧边栏，路由级登录守卫） */}
            <Route
              element={
                <RequireAuth>
                  <DashboardLayout area="developer" />
                </RequireAuth>
              }
            >
              <Route path="me" element={<DeveloperDashboard />} />
              <Route path="me/plugins" element={<MyPlugins />} />
              <Route path="me/plugins/new" element={<SubmitRoute mode="new" />} />
              <Route path="me/plugins/:pluginId/edit" element={<SubmitRoute mode="edit" />} />
              <Route path="me/plugins/:pluginId/new-version" element={<SubmitRoute mode="newVersion" />} />
              <Route path="me/plugins/:pluginId" element={<WorkspaceRoute />} />
            </Route>
            {/* 管理后台（侧边栏，路由级管理员守卫） */}
            <Route
              element={
                <RequireAdmin>
                  <DashboardLayout area="admin" />
                </RequireAdmin>
              }
            >
              <Route path="admin" element={<AdminDashboard />} />
              <Route path="admin/review" element={<ReviewQueue />} />
              <Route path="admin/plugins" element={<AdminPlugins />} />
              <Route path="admin/official/new" element={<SubmitRoute mode="official" />} />
              <Route
                path="admin/plugins/:pluginId/official-version"
                element={<SubmitRoute mode="officialVersion" />}
              />
              <Route path="admin/developers" element={<AdminDevelopers />} />
              <Route path="admin/settings" element={<AdminSettings />} />
              <Route path="admin/audit" element={<AdminAudit />} />
            </Route>
            <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </Suspense>
        </ToastProvider>
      </AuthProvider>
      </BrowserRouter>
    </ErrorBoundary>
  )
}
