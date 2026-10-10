import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwind from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

export default defineConfig({
  base: "/admin/",
  plugins: [react(), tailwind()],
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  server: {
    proxy: {
      "/api": {
        target: `http://${process.env.UNDERSTAND_BOOK_ADDR ?? "127.0.0.1:8788"}`,
        changeOrigin: false,
      },
    },
  },
  test: { include: ["src/**/*.test.ts"] },
});
