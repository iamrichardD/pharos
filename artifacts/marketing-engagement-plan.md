/* ========================================================================
 * Project: pharos
 * Component: Marketing & Documentation
 * File: marketing-engagement-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Research and design document for measuring user engagement on the 
 * Pharos marketing site (website/) without compromising user privacy.
 * * Traceability:
 * Related to GitHub Issue #145, Task 24.1.
 * ======================================================================== */

# Marketing Engagement Measurement Strategy

This document outlines the strategy for quantifying engagement on the Pharos marketing site to evaluate the effectiveness of the "Open Source Advocate" persona and the "Zero-Friction Discovery" goal.

## 1. Engagement Audit & Interaction Points

The following high-signal interaction points have been identified within the `website/` component:

### 1.1 "One-Liner" Copy Events
*   **Location:** `website/src/components/SandboxSnippet.astro` (id: `copy-sandbox`) and `website/src/content/docs/install.mdx`.
*   **Signal:** High intent to evaluate or install Pharos.
*   **Measurement:** Track `click` events on copy buttons.

### 1.2 Tab Interaction (Persona Split)
*   **Location:** `website/src/components/TieredTabs.astro`.
*   **Signal:** Identifies the user persona (Home Lab vs. Enterprise).
*   **Measurement:** Track `click` events on tab buttons with `data-tab` attributes.

### 1.3 Deep Reads (Protocol Engagement)
*   **Location:** `website/src/content/docs/architecture.mdx` and any future RFC-specific pages.
*   **Signal:** Interest in the underlying technical standards (RFC 2378).
*   **Measurement:** Scroll depth (25%, 50%, 75%, 100%) and time-on-page.

---

## 2. Proposed Approaches

### Approach A: Static / No-JS (Tier 1)
Focuses on absolute privacy and zero client-side overhead.

*   **Mechanism:** Server-side log analysis (via GitHub Pages/Vercel) or CSS "Pixel" triggers.
*   **Implementation:** 
    *   Use CSS `:active` or `:focus` states to trigger a request to a tracking endpoint (e.g., `url(/api/track?event=cta-click)`).
    *   Pros: No JavaScript required, impossible to block with standard ad-blockers, 100% privacy-preserving.
    *   Cons: Cannot track complex client-side interactions (like "copy to clipboard" success) or scroll depth.

### Approach B: SaaS Analytics (Tier 2 - Recommended)
Focuses on lightweight, cookie-less, and privacy-first SaaS vendors.

*   **Primary Candidate:** **Umami Cloud**
    *   **Payload Size:** ~2kB.
    *   **Privacy:** Cookie-less, no personal data collected, GDPR/CCPA compliant.
    *   **Free Tier:** 100,000 monthly events (Generous for OSS).
*   **Secondary Candidate:** **PostHog**
    *   **Payload Size:** ~50kB (includes heatmaps/recordings).
    *   **Free Tier:** 1,000,000 monthly events.
    *   **Value:** Best for understanding "where users get stuck" via session recordings.

---

## 3. Vendor Free-Tier Matrix

| Vendor | Free Tier Policy | Event Limit | Cookie-less? | Payload | Link |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Umami** | Hobby (Free) | 100k / mo | Yes | ~2kB | [umami.is](https://umami.is/pricing) |
| **PostHog** | Generous Free | 1M / mo | Yes | ~50kB | [posthog.com](https://posthog.com/pricing) |
| **Plausible** | Self-Host Only | N/A | Yes | ~1kB | [plausible.io](https://plausible.io/self-hosted) |

---

## 4. Key Performance Indicators (KPIs)

1.  **CLI-Copy-Conversion:** Total "Copy" clicks on installation commands divided by unique visitors. Target: >5%.
2.  **Persona-Split:** Percentage of users viewing "Home Lab" vs. "Enterprise" content. Target: 70/30 split.
3.  **Deep-Read-Ratio:** Percentage of users reaching 75% scroll depth on the Architecture/RFC pages. Target: >15%.
4.  **Sandbox-Velocity:** Time from landing on homepage to clicking the Sandbox copy button. Target: <30 seconds.

---

## 5. Implementation Roadmap

1.  **Phase 1:** Provision an **Umami Cloud** account.
2.  **Phase 2:** Integrate the Umami script tag in `website/src/layouts/BaseLayout.astro`.
3.  **Phase 3:** Instrument `SandboxSnippet.astro` and `TieredTabs.astro` with custom event triggers (e.g., `umami.track('copy-sandbox')`).
4.  **Phase 4:** Configure a custom dashboard to visualize the Persona-Split and Copy-Conversion.
