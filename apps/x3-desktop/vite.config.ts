import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

const host = process.env.TAURI_DEV_HOST;
const domain = process.env.VITE_DOMAIN || "x3star.net";

export default defineConfig({
  plugins: [
    react(),
  ],
  optimizeDeps: {
    // Avoid dependency discovery to reduce Rolldown pre-bundle issues in Vite 8.
    noDiscovery: true,
    include: ["buffer", "process"],
  },
  define: {
    global: "globalThis",
    "process.env": {},
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      buffer: "buffer",
      process: "process/browser",
    },
  },
  clearScreen: false,
  server: {
    port: 5173,
    // Allow connections from Cloudflare Tunnel and local Tauri
    host: host || "0.0.0.0",
    hmr: host ? { protocol: "ws", host, port: 5173 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
    // Allow the tunnel domain and localhost through Vite's host check
    allowedHosts: [domain, `www.${domain}`, "localhost", "127.0.0.1"],
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "es2022",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
