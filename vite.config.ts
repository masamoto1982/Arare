import { defineConfig } from 'vite';
import { readFileSync } from 'node:fs';

const isTauri = !!process.env.TAURI_ENV_TARGET_TRIPLE;
const target = process.env.AJISAI_BUILD_TARGET === 'tauri' ? 'tauri' : 'web';

// The release version the four synced manifests declare. `package.json` is the
// source of truth `check:version-sync` holds the others to, so reading it here
// keeps the badge's tooltip from becoming a fifth place to forget to update.
function releaseVersion(): string {
  return JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')).version;
}

function buildTimestampLabel(): string {
  const now = new Date();
  const year = now.getFullYear();
  const month = `${now.getMonth() + 1}`.padStart(2, '0');
  const day = `${now.getDate()}`.padStart(2, '0');
  const hours = `${now.getHours()}`.padStart(2, '0');
  const minutes = `${now.getMinutes()}`.padStart(2, '0');
  return `${year}${month}${day}${hours}${minutes}`;
}

export default defineConfig({
  root: '.',
  base: target === 'tauri' ? '/' : './',
  define: {
    __AJISAI_TARGET__: JSON.stringify(target),
    __AJISAI_BUILD_TIMESTAMP__: JSON.stringify(buildTimestampLabel()),
    __AJISAI_RELEASE_VERSION__: JSON.stringify(releaseVersion())
  },
  // Cross-origin isolation for SharedArrayBuffer-backed wasm threading
  // (implicit-parallelism roadmap Phase 5). These response headers make
  // `crossOriginIsolated` true under `npm run dev` / `vite preview`, so a
  // threaded wasm build can be exercised locally. They apply to the dev and
  // preview servers only; `vite build` emits static files and does not bake
  // headers in, so the GitHub Pages deployment stays cross-origin unisolated
  // (no service-worker fallback is registered) and always runs
  // single-threaded there. Tauri runs in a native WebView that does not need
  // them.
  server: {
    host: '0.0.0.0',
    port: 3000,
    open: false,
    strictPort: true,
    headers: isTauri
      ? {}
      : {
          'Cross-Origin-Opener-Policy': 'same-origin',
          'Cross-Origin-Embedder-Policy': 'require-corp'
        }
  },
  preview: {
    headers: isTauri
      ? {}
      : {
          'Cross-Origin-Opener-Policy': 'same-origin',
          'Cross-Origin-Embedder-Policy': 'require-corp'
        }
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: true,
    rollupOptions: {
      // Resolved at runtime inside the Tauri WebView; never bundled. The
      // platform/tauri.ts adapter uses `import(/* @vite-ignore */ ...)`
      // so these specifiers must remain external for both web and Tauri
      // builds.
      external: [
        '@tauri-apps/api/core',
        '@tauri-apps/api/event',
        '@tauri-apps/plugin-dialog',
        '@tauri-apps/plugin-fs',
        '@tauri-apps/plugin-store',
        /^@tauri-apps\//
      ]
    }
  },
  optimizeDeps: {
    exclude: ['./src/wasm/generated/ajisai_core.js']
  },
  resolve: {
    alias: {
      '@': '/src'
    }
  },
  worker: {
    format: 'es'
  },
  publicDir: 'public'
});
