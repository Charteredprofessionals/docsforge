import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: false,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          // Vendor chunks - split major dependencies
          "vendor-react": ["react", "react-dom"],
          "vendor-icons": ["lucide-react"],
          "vendor-mammoth": ["mammoth"],
          "vendor-uuid": ["uuid"],
        },
      },
    },
    // Enable brotli/gzip pre-compression for smaller assets
    assetsInlineLimit: 4096,
    chunkSizeWarningLimit: 1000,
  },
});