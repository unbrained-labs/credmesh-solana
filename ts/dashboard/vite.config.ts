import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: { port: 5173 },
  resolve: {
    alias: {
      // wallet-adapter / web3.js reference Node's `buffer` — alias to the npm polyfill
      buffer: 'buffer',
    },
  },
  define: {
    // some bundled deps probe for `global` (Node)
    global: 'globalThis',
  },
  optimizeDeps: {
    include: ['buffer'],
  },
});
