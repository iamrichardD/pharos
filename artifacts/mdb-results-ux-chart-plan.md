/* ========================================================================
 * Project: pharos
 * Component: Web Console / Planning
 * File: artifacts/mdb-results-ux-chart-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This document outlines the user experience (UX) update for the MDB search
 * results page. It transitions from individual card panels to a data-driven
 * green bar chart visualization, improving visual scanability and providing
 * pagination for large datasets.
 * * Traceability:
 * Related to Phase 16 (Issue #64) and Project Mandate for Rich Aesthetics.
 * ======================================================================== */

# Feature Plan: MDB Results Chart & Pagination

## 1. Objective
To enhance the readability and scanability of MDB search results in the Pharos Web Console by replacing the individual grid panels with a centralized green bar chart and implementing pagination (default 25 records).

## 2. User Experience Design

### 2.1 Visualization: The Green Bar Chart
Instead of rendering each machine record as a separate card, results will be aggregated or listed in a high-density bar chart.
- **Metric**: The X-axis will represent record identifiers (e.g., Hostname or ID), and the Y-axis (bar height) can represent key infrastructure metrics like `cpu` or `mem_used` (data permitting).
- **Styling**: 
    - **Color**: Strict adherence to the "Pharos Green" theme (e.g., Tailwind `emerald-500` or `green-500`).
    - **Interactivity**: Hovering over a bar will display a tooltip with the full record details (IP, MAC, Owner, Status).
    - **Empty State**: Maintain a clear "No records found" message.

### 2.2 Navigation: Pagination
Large environments can have hundreds or thousands of machine records.
- **Default Page Size**: 25 records per page.
- **UI Elements**: 
    - "Previous" and "Next" buttons.
    - Page count display (e.g., "Showing 1-25 of 142 records").
    - Jump-to-page input for rapid navigation.
- **Implementation**: The pagination logic will reside in the `search-logic.ts` slice, utilizing server-side filtering if possible, or client-side slicing for the MVP.

## 3. Technical Strategy (Vertical Slice Architecture)

### 3.1 Data Layer (`search-logic.ts`)
- Update the `searchMdb` function to support `page` and `pageSize` parameters.
- If the Pharos Server protocol (RFC 2378) does not yet support server-side pagination, the logic will perform client-side slicing of the full `PharosResponse`.

### 3.2 UI Layer (`MdbResultsChart.astro`)
- Create a new component `MdbResultsChart.astro` using a lightweight charting library (e.g., `Chart.js` with a headless wrapper or pure SVG/CSS bars for zero-dependency speed).
- Deprecate (but keep for reference) `MdbResultsGrid.astro`.

### 3.3 State Management
- Use URL Search Parameters (`?q=...&p=2&limit=25`) to ensure page state is shareable and survives refreshes.

## 4. Visual Layout Mockup (Mermaid)

```mermaid
graph TD
    subgraph WebConsole ["Web Console: /mdb"]
        Header[Search MDB Header]
        Form[Search Input Form]
        
        subgraph ResultsArea ["Results Area"]
            Chart[Green Bar Chart: Records Visualization]
            Pager[Pagination: Prev | 1 2 3 ... | Next]
        end
        
        Header --> Form
        Form --> ResultsArea
    end
```

## 5. Verification Strategy

### 5.1 Unit Tests
- `search-logic.test.ts`: Verify that slicing logic returns the correct subset of records for page 1, page 2, etc.

### 5.2 Manual Verification
- Perform a broad query (e.g., `*`) in the sandbox environment.
- Verify that only 25 records are shown by default.
- Verify that clicking "Next" updates the chart with the subsequent batch.
- Verify the green aesthetic matches the Pharos design system.
