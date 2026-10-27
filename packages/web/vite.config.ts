import { configDefaults, defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
import { pdfOcrAssets } from "./pdf-ocr-assets.mjs";
import { boardPreviewAssets } from "./board-preview-assets.mjs";
import { fileURLToPath } from "node:url";

// dev proxy:前端 `/api/*` → tiny_http server(同源、无 CORS,承 ADR-0022)。
// 打包形态(S10e)由 tiny_http 同端口服 dist + API,proxy 仅 dev 用。
const backend = process.env.UNDERSTAND_BOOK_ADDR
  ? `http://${process.env.UNDERSTAND_BOOK_ADDR}`
  : "http://127.0.0.1:8787";

export default defineConfig(({ mode }) => ({
  define: mode === "multi" ? { "import.meta.env.VITE_MULTI_USER": JSON.stringify("1") } : {},
  plugins: [vue(), pdfOcrAssets(), boardPreviewAssets(), {
    name: "reader-build-mode",
    apply: "build",
    transformIndexHtml: () => [{ tag: "meta", attrs: {
      name: "understand-book-mode",
      content: mode === "multi" || process.env.VITE_MULTI_USER === "1" ? "multi-user" : "local",
    }, injectTo: "head" }],
  }, {
    name: "reader-startup-styles",
    apply: "build",
    transformIndexHtml: {
      order: "post",
      handler: (html, context) => context.path === "/home.html" ? html : html.replace(/<link\b[^>]*rel="stylesheet"[^>]*>/g,
        tag => tag.replace('rel="stylesheet"', 'rel="preload" as="style" data-app-styles')),
    },
  }],
  build: {
    rollupOptions: {
      input: {
        reader: fileURLToPath(new URL("./index.html", import.meta.url)),
        home: fileURLToPath(new URL("./home.html", import.meta.url)),
      },
    },
  },
  test: {
    exclude: [...configDefaults.exclude, "playwright/**"],
  },
  server: {
    proxy: {
      "/api": {
        target: backend,
        changeOrigin: true,
        rewrite: (p) => p.replace(/^\/api/, ""),
      },
    },
  },
}));
