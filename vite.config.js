import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    rolldownOptions: {
      input: {
        game: fileURLToPath(new URL('./index.html', import.meta.url)),
        catAnimation: fileURLToPath(new URL('./cat-animation.html', import.meta.url)),
      },
    },
  },
});
