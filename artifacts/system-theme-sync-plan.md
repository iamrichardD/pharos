# Feature Plan: Browser-Synced Theme Management (System Preference)

/* ========================================================================
 * Project: pharos
 * Component: Marketing Site - Architecture
 * File: system-theme-sync-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Documenting the shift from manual-only theme toggling to system-synced 
 * color scheme management for improved UX and accessibility.
 * * Traceability:
 * Related to GitHub Issue #58 (TBD), enhances Phase 14/15 UX goals.
 * ======================================================================== */

## 1. Executive Summary
The goal of this feature is to transition the Pharos marketing site from a user-action-triggered theme toggle to a browser-centric synchronization model. The site will automatically adapt to the user's operating system color scheme (`prefers-color-scheme`) in real-time, while still allowing for a persistent user override if desired.

## 2. Technical Implementation Plan

### 2.1 Head Script Refactor (`BaseLayout.astro`)
The current in-lined script will be simplified to prioritize the browser's media query over `localStorage` for a "System First" experience.

**Key Changes:**
*   Establish `window.matchMedia('(prefers-color-scheme: dark)')` as the primary source of truth.
*   Implement a `change` event listener on the media query to ensure the `dark` class is toggled on the `<html>` element instantly when the OS setting changes, without requiring a page refresh.

### 2.2 Toggle Component Enhancement (`DarkModeToggle.astro`)
The `ThemeToggle` TypeScript class will be updated to handle the "System" state gracefully.

**Key Changes:**
*   **Automatic Sync:** The component will listen for system theme changes and update its internal state and icons accordingly.
*   **User Intent:** If a user manually clicks the toggle, a `theme` key will be saved to `localStorage` to respect their explicit preference.
*   **System Reversion:** (Optional) Implement a "Follow System" option or clear the `localStorage` key if the user toggles back to the system-matching state, returning control to the browser.

### 2.3 UX & Accessibility
*   **FOUC Prevention:** The head script remains in-lined to prevent a "Flash of Unstyled Content" where the site briefly shows light mode before switching to dark mode on load.
*   **Smooth Transitions:** Ensure Tailwind's `transition-colors` durations are consistent across the layout to provide a polished feel during automatic transitions.

## 3. Verification Protocol

### 3.1 Development Verification (Zero-Host)
1.  Launch the debug container: `podman run --rm -it -v $(pwd):/app:Z pharos-debug`.
2.  Verify the `astro check` and `build` pass with the new logic.

### 3.2 Manual & Environment Verification
1.  **OS Sync:** Open the site and toggle the OS appearance (Light/Dark). The site must update immediately.
2.  **Persistence:** Manually set the theme to Dark on a Light OS. Refresh the page. The site must remain Dark (respecting user override).
3.  **DevTools Emulation:** Use the Chrome/Firefox "Emulate CSS prefers-color-scheme" feature to verify behavior across different preference states.

