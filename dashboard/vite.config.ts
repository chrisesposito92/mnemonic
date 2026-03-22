import { defineConfig } from 'vite'
import preact from '@preact/preset-vite'
import tailwindcss from '@tailwindcss/vite'
import { viteSingleFile } from 'vite-plugin-singlefile'

export default defineConfig({
  plugins: [
    preact(),
    tailwindcss(),
    viteSingleFile(),
  ],
  base: '/ui/',
  server: {
    port: 5173,
    proxy: {
      '/health': 'http://localhost:8080',
    },
  },
  build: {
    outDir: 'dist',
    target: 'esnext',
  },
})
