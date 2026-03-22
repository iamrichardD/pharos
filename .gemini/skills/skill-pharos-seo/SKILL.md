---
name: skill-pharos-seo
description: Manage SEO (Search Engine Optimization) and AIO (AI Agent Search Optimization) for Project Pharos. Use this skill when auditing, modifying, or creating website content, documentation, or when requested to ensure AI/LLM readiness (llms.txt, robots.txt) and human SEO compliance.
---

# Pharos SEO & AIO (AI Agent Search Optimization)

## Overview

This skill ensures that the Pharos documentation and marketing sites are fully optimized for both human discovery (Standard SEO) and autonomous agent retrieval (AIO - AI Agent Search Optimization). Pharos's mission is to eliminate the "Hallucination Gap", so its own digital presence must be deterministically structured for LLM ingestion via `llms.txt`, clear semantic markup, and predictable chunking.

## Core Capabilities & Workflows

### 1. The Auditor Workflow
When asked to perform an SEO/AIO audit, you must:
1. Run the bundled script: `node .gemini/skills/skill-pharos-seo/scripts/audit-aio.js` to identify missing essential files (e.g., `llms.txt`, `llms-full.txt`, `robots.txt`).
2. Verify that OpenGraph and Twitter cards are present in the `<head>` of Astro pages (specifically in `BaseLayout.astro`).
3. Check the Semantic Guardrails: Ensure no "banned" terminology is used (e.g., "Single Source of Truth" should be "Unified Source of Truth").

### 2. The Generator Workflow
When asked to generate AIO/SEO resources:
1. **llms.txt**: Ensure `website/public` contains an `llms.txt` and `llms-full.txt`. These MUST follow the [llms.txt standard](https://llmstxt.org/), providing clear Markdown summaries and links to raw documentation files.
2. **Robots**: Ensure `website/public/robots.txt` exists and points to the sitemap.
3. **Structured Data**: For standard SEO, ensure `BaseLayout.astro` or page-specific components include appropriate `JSON-LD` schemas (e.g., representing Pharos as a `SoftwareApplication`).

### 3. Documentation Chunking & Structure
When reviewing or creating new files in `docs/`:
- **Semantic Headers:** Strictly use `##` and `###` headers sequentially. Do not skip header levels.
- **RAG-Ready Lists:** Favor bulleted lists over long, unwieldy paragraphs to ensure high-fidelity chunking by vector databases.
- **Concise Scope:** Keep individual markdown files focused on a single architectural slice or workflow to reduce context bloat during subsequent LLM retrieval.

## Semantic Guardrails (Project Terminology)
You must enforce the use of these exact terms when describing Pharos features to ensure consistent AI grounding:
- **Unified Source of Truth** (DO NOT use "single source of truth")
- **Hallucination Gap** (Refers to LLMs hallucinating infrastructure states)
- **Deterministic Infrastructure**
- **webMCP Grounding Layer**

If you spot variations during an audit, you must propose or apply a correction.
