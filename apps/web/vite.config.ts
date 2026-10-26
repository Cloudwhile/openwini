import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
  server: {
    port: 3000,
    strictPort: true,
    proxy: {
      // 开发环境把 /api 代理到后端；生产由 Rust 服务统一提供页面和 API。
      "/api": {
        target: process.env.API_PROXY_TARGET || "http://localhost:8080",
        changeOrigin: true,
      },
    },
  },
})
