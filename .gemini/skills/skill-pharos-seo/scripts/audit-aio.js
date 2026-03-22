#!/usr/bin/env node
/* ========================================================================
 * Project: pharos
 * Component: Skill - Pharos SEO
 * File: audit-aio.js
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Automated AIO (AI Agent Search Optimization) and SEO auditor for the 
 * Pharos marketing site and documentation.
 * * Traceability:
 * Related to the implementation of skill-pharos-seo.
 * ======================================================================== */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const PROJECT_ROOT = path.resolve(__dirname, '../../../..');
const WEBSITE_PUBLIC = path.join(PROJECT_ROOT, 'website', 'public');
const WEBSITE_SRC = path.join(PROJECT_ROOT, 'website', 'src');

console.log('🔍 Starting Pharos SEO & AIO Audit...\n');

let hasErrors = false;
let hasWarnings = false;

// 1. Check for Essential AIO/SEO Files
const essentialFiles = [
  { path: path.join(WEBSITE_PUBLIC, 'llms.txt'), type: 'AIO' },
  { path: path.join(WEBSITE_PUBLIC, 'llms-full.txt'), type: 'AIO' },
  { path: path.join(WEBSITE_PUBLIC, 'robots.txt'), type: 'SEO' }
];

console.log('--- 📄 File Checks ---');
for (const file of essentialFiles) {
  if (fs.existsSync(file.path)) {
    console.log(`✅ [${file.type}] Found: ${path.basename(file.path)}`);
  } else {
    console.error(`❌ [${file.type}] Missing: ${path.basename(file.path)}`);
    hasErrors = true;
  }
}

// 2. Semantic Guardrail Checks
console.log('\n--- 🧠 Semantic Guardrails ---');
try {
  // Use ripgrep or standard grep to check for "Single Source of Truth" in website/src and docs
  const cmd = `grep -inr "Single Source of Truth" ${WEBSITE_SRC} ${path.join(PROJECT_ROOT, 'docs')} || true`;
  const result = execSync(cmd, { encoding: 'utf-8' }).trim();
  
  if (result) {
    console.warn(`⚠️  WARNING: Found "Single Source of Truth". This should be changed to "Unified Source of Truth".`);
    console.warn(result);
    hasWarnings = true;
  } else {
    console.log(`✅ All clear: No instances of "Single Source of Truth" found.`);
  }
} catch (e) {
  // If grep fails (e.g. dir doesn't exist), just ignore for the basic audit
}

// 3. OpenGraph / Twitter Meta Check in BaseLayout
console.log('\n--- 🌐 OpenGraph / SEO Tags ---');
const baseLayoutPath = path.join(WEBSITE_SRC, 'layouts', 'BaseLayout.astro');
if (fs.existsSync(baseLayoutPath)) {
  const content = fs.readFileSync(baseLayoutPath, 'utf-8');
  if (content.includes('property="og:title"') && content.includes('name="twitter:card"')) {
    console.log(`✅ OpenGraph and Twitter tags found in BaseLayout.astro`);
  } else {
    console.warn(`⚠️  WARNING: Missing OpenGraph (og:title) or Twitter cards in BaseLayout.astro`);
    hasWarnings = true;
  }
  
  if (content.includes('type="application/ld+json"')) {
    console.log(`✅ JSON-LD structured data found in BaseLayout.astro`);
  } else {
    console.warn(`⚠️  WARNING: Missing JSON-LD structured data in BaseLayout.astro`);
    hasWarnings = true;
  }
} else {
  console.log(`⏭️  Skipping Layout Check: BaseLayout.astro not found.`);
}

console.log('\n--- 📊 Audit Summary ---');
if (hasErrors) {
  console.log('❌ Audit failed. Please resolve missing essential files.');
  process.exit(1);
} else if (hasWarnings) {
  console.log('⚠️  Audit passed with warnings. Please review the semantic and meta tag warnings.');
  process.exit(0); // Warnings don't block CI natively, but prompt AI action
} else {
  console.log('✅ All AIO and SEO checks passed! The infrastructure is deterministic.');
  process.exit(0);
}
