import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Relative base paths so the build works under any GitHub Pages path
// (USER.github.io, USER.github.io/REPO, or a custom domain).
// If you prefer absolute paths, set this to '/your-repo-name/'.
export default defineConfig({
  plugins: [react()],
  base: './',
  build: {
    outDir: 'dist',
    sourcemap: false,
    assetsInlineLimit: 4096,
  },
});
