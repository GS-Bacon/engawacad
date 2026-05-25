import { defineConfig } from "vite";

export default defineConfig({
  base: "./",
  build: {
    outDir: "dist",
  },
  server: {
    proxy: {
      "/api": {
        target: process.env.VITE_API_TARGET ?? "http://127.0.0.1:3000",
        changeOrigin: true,
      },
    },
  },
  test: {
    include: ["src/**/*.test.ts"],
  },
});
