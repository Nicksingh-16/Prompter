import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  plugins: [react()],
  build: {
    rollupOptions: {
      input: {
        main:   resolve(__dirname, 'index.html'),
        loader: resolve(__dirname, 'loader.html'),
        pill:   resolve(__dirname, 'pill.html'),
      },
    },
  },
})