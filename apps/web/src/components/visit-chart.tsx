import { useEffect, useMemo, useRef } from "react"
import * as echarts from "echarts/core"
import { LineChart } from "echarts/charts"
import { GridComponent, LegendComponent, TooltipComponent } from "echarts/components"
import { CanvasRenderer } from "echarts/renderers"
import { useTheme } from "@/components/theme-provider"
import { t } from "@/i18n"

// 按需注册：仅折线图 + 网格/提示/图例 + Canvas 渲染，控制打包体积
echarts.use([LineChart, GridComponent, LegendComponent, TooltipComponent, CanvasRenderer])

export interface VisitPoint {
  day: string
  pv: number
  uv: number
}

/**
 * 近 N 天访问图（ECharts 柱状图）：
 * PV（品牌色）+ UV（成功色）双系列，跟随明暗主题取色，容器尺寸自适应。
 */
export function VisitChart({ series, days }: { series: VisitPoint[]; days: number }) {
  const { theme } = useTheme()
  const containerRef = useRef<HTMLDivElement>(null)
  const chartRef = useRef<echarts.ECharts | null>(null)

  // 缺失日期补零，保证 x 轴连续
  const full = useMemo(() => {
    const byDay = new Map(series.map((s) => [s.day, s]))
    const out: VisitPoint[] = []
    const today = new Date()
    for (let i = days - 1; i >= 0; i--) {
      const d = new Date(today)
      d.setDate(d.getDate() - i)
      const key = d.toISOString().slice(0, 10)
      out.push(byDay.get(key) ?? { day: key, pv: 0, uv: 0 })
    }
    return out
  }, [series, days])

  // 初始化 / 销毁（一次）
  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const chart = echarts.init(el)
    chartRef.current = chart
    const ro = new ResizeObserver(() => chart.resize())
    ro.observe(el)
    return () => {
      ro.disconnect()
      chart.dispose()
      chartRef.current = null
    }
  }, [])

  // 数据或主题变化时重设配置（颜色从 CSS 变量读取，跟随当前主题）
  useEffect(() => {
    const chart = chartRef.current
    if (!chart) return
    const css = getComputedStyle(document.documentElement)
    const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback
    chart.setOption({
      animationDuration: 200,
      grid: { left: 8, right: 8, top: 30, bottom: 0, containLabel: true },
      legend: {
        top: 0,
        left: 0,
        icon: "roundRect",
        itemWidth: 10,
        itemHeight: 10,
        itemGap: 16,
        textStyle: { color: v("--muted-foreground", "#888"), fontSize: 11 },
      },
      tooltip: {
        trigger: "axis",
        axisPointer: { type: "shadow" },
        backgroundColor: v("--popover", "#fff"),
        borderColor: v("--border", "#ddd"),
        textStyle: { color: v("--popover-foreground", "#222"), fontSize: 12 },
      },
      xAxis: {
        type: "category",
        data: full.map((p) => p.day.slice(5)),
        axisTick: { show: false },
        axisLine: { lineStyle: { color: v("--border", "#ddd") } },
        axisLabel: { color: v("--muted-foreground", "#888"), fontSize: 10 },
      },
      yAxis: {
        type: "value",
        minInterval: 1,
        splitLine: { lineStyle: { color: v("--border", "#ddd") } },
        axisLabel: { color: v("--muted-foreground", "#888"), fontSize: 10 },
      },
      series: [
        {
          name: t.stats.pv,
          type: "line",
          smooth: true,
          symbol: "circle",
          symbolSize: 5,
          showSymbol: false,
          data: full.map((p) => p.pv),
          lineStyle: { width: 2, color: v("--primary", "#5b8def") },
          itemStyle: { color: v("--primary", "#5b8def") },
          areaStyle: { color: v("--primary", "#5b8def"), opacity: 0.1 },
        },
        {
          name: t.stats.uv,
          type: "line",
          smooth: true,
          symbol: "circle",
          symbolSize: 5,
          showSymbol: false,
          data: full.map((p) => p.uv),
          lineStyle: { width: 2, color: v("--success", "#3fbf7f") },
          itemStyle: { color: v("--success", "#3fbf7f") },
          areaStyle: { color: v("--success", "#3fbf7f"), opacity: 0.1 },
        },
      ],
    })
  }, [full, theme])

  return (
    <div
      ref={containerRef}
      className="h-44 w-full"
      role="img"
      aria-label={`${t.stats.pv} / ${t.stats.uv}`}
    />
  )
}
