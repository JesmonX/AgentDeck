import { defineConfig } from "vite";
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:47831",
        headers: {
          Authorization: `Bearer ${process.env.AGENTDECK_DEV_TOKEN ?? ""}`,
        },
      },
    },
  },
  build: { outDir: "dist", chunkSizeWarningLimit: 1600 },
});
