import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],

  build: {
    outDir: '../backend-rust/src/static',
    emptyOutDir: true,
    rollupOptions: {
      output: {
        entryFileNames: `assets/[name].js`,
        chunkFileNames: `assets/[name].js`,
        assetFileNames: `assets/[name].[ext]`,
      },
    },
  },

  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:80',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, 'api'),
      },
    },
  },
});
