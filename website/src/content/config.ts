/* ========================================================================
 * Project: pharos
 * Component: Marketing Site
 * File: config.ts
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Content collection configuration for the Pharos documentation.
 * * Traceability:
 * Related to GitHub Issue #28, implements the documentation port to MDX.
 * ======================================================================== */
import { defineCollection, z } from 'astro:content';

const docs = defineCollection({
  type: 'content',
  schema: z.object({
    title: z.string(),
    description: z.string().optional(),
    order: z.number().optional(),
  }),
});

export const collections = {
  docs,
};
