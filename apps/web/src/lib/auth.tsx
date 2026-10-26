import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react"
import { api, setCsrfToken, type Me } from "./api"

interface AuthCtx {
  me: Me | null
  loading: boolean
  refresh: () => Promise<void>
}

const Ctx = createContext<AuthCtx>({ me: null, loading: true, refresh: async () => {} })

export function AuthProvider({ children }: { children: ReactNode }) {
  const [me, setMe] = useState<Me | null>(null)
  const [loading, setLoading] = useState(true)

  const refresh = useCallback(async () => {
    try {
      const m = await api.get<Me>("/api/auth/me")
      setCsrfToken(m.csrfToken ?? null)
      setMe(m)
    } catch {
      setCsrfToken(null)
      setMe({ authenticated: false })
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    let alive = true
    api
      .get<Me>("/api/auth/me")
      .then((m) => {
        if (!alive) return
        setCsrfToken(m.csrfToken ?? null)
        setMe(m)
        setLoading(false)
      })
      .catch(() => {
        if (!alive) return
        setCsrfToken(null)
        setMe({ authenticated: false })
        setLoading(false)
      })
    return () => {
      alive = false
    }
  }, [])

  return <Ctx.Provider value={{ me, loading, refresh }}>{children}</Ctx.Provider>
}

export function useAuth() {
  return useContext(Ctx)
}

export interface RequireAuthState {
  ready: boolean
  logged: boolean
  me: Me | null
}

export function useRequireAuth(): RequireAuthState {
  const { me, loading } = useAuth()
  return { ready: !loading, logged: !!me?.authenticated, me }
}

export function useRequireAdmin() {
  const { me, loading } = useAuth()
  return { ready: !loading, logged: !!me?.authenticated, isAdmin: !!me?.developer?.isAdmin, me }
}
