import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { execSync } from 'node:child_process'

let buildCommit = ''
try {
  buildCommit = execSync('git rev-parse --short HEAD', { stdio: ['ignore', 'pipe', 'ignore'] })
    .toString()
    .trim()
} catch {
  buildCommit = 'unknown'
}

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  define: {
    __BUILD_COMMIT__: JSON.stringify(buildCommit)
  },
  server: {
    port: 5173,
    strictPort: true
  },
  build: {
    target: 'chrome110',
    chunkSizeWarningLimit: 1500
  }
})
