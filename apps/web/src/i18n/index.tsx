import { useSyncExternalStore } from "react"
import { zh, type Lang } from "./zh"
import { en } from "./en"

export type LangCode = "zh" | "en"
export type { Lang }

const STORAGE_KEY = "lang"

function initialLang(): LangCode {
  const saved = localStorage.getItem(STORAGE_KEY)
  return saved === "en" ? "en" : "zh"
}

let current: LangCode = initialLang()
const listeners = new Set<() => void>()

export function getLang(): LangCode {
  return current
}

export function setLang(lang: LangCode) {
  if (lang === current) return
  current = lang
  localStorage.setItem(STORAGE_KEY, lang)
  document.documentElement.lang = lang === "zh" ? "zh-CN" : "en"
  listeners.forEach((f) => f())
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** 订阅语言变化（切换语言时配合 App 级 key 重挂载使用） */
export function useLang(): LangCode {
  return useSyncExternalStore(subscribe, getLang)
}

const dicts: Record<LangCode, Lang> = { zh, en }

/** 模块级文案代理：取值时按当前语言解析；语言切换由 App 根节点 key 重挂载触发重渲染 */
export const t: Lang = new Proxy({} as Lang, {
  get: (_target, prop: string) => (dicts[current] as unknown as Record<string, unknown>)[prop],
})
