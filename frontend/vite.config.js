import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Output filenames are fixed (no content hash) so the Rust server can embed
// them with include_str! and serve them from known routes.
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: '../static/dist',
    emptyOutDir: true,
    rollupOptions: {
      output: {
        entryFileNames: 'app.js',
        chunkFileNames: 'chunk-[name].js',
        assetFileNames: 'app.[ext]',
      },
    },
  },
  server: {
    // `npm run dev` proxies API/WS traffic to the Rust server on :3000.
    proxy: {
      '/ws': { target: 'ws://localhost:3000', ws: true },
      '/auth': 'http://localhost:3000',
      '/api': 'http://localhost:3000',
      '/stats': 'http://localhost:3000',
    },
  },
});
