/* ========================================================================
 * Project: pharos
 * Component: Web Console
 * File: astro.config.mjs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Configuration file for the Pharos Web Console. Sets the project to SSR 
 * mode (Node.js adapter) and enables Tailwind CSS for responsive styling.
 * * Traceability:
 * Related to Task 16.1 in Phase 16.
 * ======================================================================== */

// @ts-check
import { defineConfig } from 'astro/config';

import node from '@astrojs/node';
import tailwindcss from '@tailwindcss/vite';

// https://astro.build/config
export default defineConfig({
  output: 'server',
  adapter: node({
    mode: 'standalone'
  }),

  vite: {
    plugins: [tailwindcss()]
  }
});