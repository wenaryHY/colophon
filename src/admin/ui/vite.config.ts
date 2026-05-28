/// <reference types="vitest/config" />
import { defineConfig } from 'vite'
import type { ViteDevServer } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

function printDevUrlPlugin() {
  return {
    name: 'print-dev-url',
    configureServer(server: ViteDevServer) {
      server.httpServer?.once('listening', () => {
        setTimeout(() => {
          console.log('\n\x1b[43m\x1b[30m======================================================\x1b[0m')
          console.log('\x1b[43m\x1b[30m ⚠️  [InkForge Dev] 注意: 请不要直接访问上方的 5173 端口 \x1b[0m')
          console.log('\x1b[43m\x1b[30m 👉 本地开发与联调的唯一正确入口是: http://127.0.0.1:2000 \x1b[0m')
          console.log('\x1b[43m\x1b[30m======================================================\x1b[0m\n')
        }, 100)
      })
    }
  }
}

export default defineConfig({
  plugins: [react(), tailwindcss(), printDevUrlPlugin()],
  base: '/admin',
  build: {
    outDir: '../dist',
    emptyOutDir: true,
  },
  server: {
    proxy: {
      '/api': 'http://127.0.0.1:2000',
      '/ws': { target: 'ws://127.0.0.1:2000', ws: true },
      '/uploads': 'http://127.0.0.1:2000',
    }
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
  }
})
