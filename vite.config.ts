import { defineConfig } from "vite";

// Porta fixa: o Tauri aponta para ela em tauri.conf.json (devUrl).
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Não vigiar o backend nem o Instant Client embutido.
      ignored: ["**/src-tauri/**", "**/runtime/**", "**/ferramentas/**"],
    },
  },
  build: {
    rollupOptions: { input: ["index.html", "execucao-trabalho.html"] },
    // O WebView2 no Windows é atual; não há necessidade de transpilar para baixo.
    target: "chrome110",
    minify: "esbuild",
    sourcemap: false,
  },
});
