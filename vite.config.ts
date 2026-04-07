import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import path from "node:path";

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src"),
    },
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes("node_modules")) {
            return;
          }

          if (id.includes("pdfjs-dist")) {
            return "pdf";
          }

          if (id.includes("@tiptap")) {
            return "editor";
          }

          if (id.includes("docx-preview")) {
            return "docx";
          }

          if (id.includes("vue-router") || id.includes("pinia")) {
            return "runtime";
          }
        },
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
  },
  clearScreen: false,
});
