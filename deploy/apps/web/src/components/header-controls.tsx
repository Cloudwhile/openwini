import { Moon, Sun } from "lucide-react"
import { Button } from "@workspace/ui/components/button"
import { useTheme } from "@/components/theme-provider"
import { getLang, setLang, t, useLang } from "@/i18n"

/** 全站共享的 Header 控件组：主题切换 + 语言切换（位置与交互在公开页/后台完全一致） */
export function HeaderControls() {
  const { theme, setTheme } = useTheme()
  const lang = useLang()
  const isDark =
    theme === "dark" ||
    (theme === "system" &&
      typeof document !== "undefined" &&
      document.documentElement.classList.contains("dark"))
  const toggleLang = () => {
    setLang(getLang() === "zh" ? "en" : "zh")
    // 语言切换整页刷新：可靠重建全部模块级文案
    window.location.reload()
  }
  return (
    <>
      <Button
        variant="ghost"
        size="icon"
        aria-label={isDark ? t.misc.themeLight : t.misc.themeDark}
        title={isDark ? t.misc.themeLight : t.misc.themeDark}
        onClick={() => setTheme(isDark ? "light" : "dark")}
      >
        {isDark ? <Sun className="size-4" /> : <Moon className="size-4" />}
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className="text-muted-foreground px-2 text-xs font-medium"
        aria-label={lang === "zh" ? "Switch to English" : "切换到中文"}
        onClick={toggleLang}
      >
        {t.misc.langToggle}
      </Button>
    </>
  )
}
