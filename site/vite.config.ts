import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'

export default defineConfig({
  // the router plugin has to come before React's: it writes src/routeTree.gen.ts from the files in src/routes
  server: { fs: { allow: ['..'] } }, // the changelog is read from the repository root
  plugins: [tanstackRouter({ target: 'react', autoCodeSplitting: true }), react(), tailwindcss()],
})
