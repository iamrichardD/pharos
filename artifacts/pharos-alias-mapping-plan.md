<!--
/* ========================================================================
 * Project: pharos
 * Component: Documentation / Planning
 * File: pharos-alias-mapping-plan.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Defines the Roadmap for supporting Field Alternation [a|b|c] and Return 
 * Coalescing to accommodate varied Enterprise Asset Management workflows 
 * while adhering to RFC 2378 where possible.
 * * Traceability:
 * Related to Phase 18 (Enterprise Workflows).
 * ======================================================================== */
-->

# Pharos Field Alternation & Mapping Plan

## 1. Problem Statement
Enterprise Asset Managers and Home Labbers often use different naming conventions for the same data (e.g., `sn`, `serial_number`, `serialNumber`). 

**Requirement**: Support a query like:
`mdb [serialNumber|serial_number|sn]=1234567 return hostname, [serialNumber|serial_number|sn]`

## 2. Proposed Feature: Field Alternation & Coalescing

### 2.1 Choice-Based Selection (Search Expansion)
When the field name in a query is a choice list `[f1|f2|f3]`, the engine performs an implicit logical `OR` search.
- **Logic**: `(f1 == value) OR (f2 == value) OR (f3 == value)`.
- **Security**: Strict tokenization against a **Permittedlist** (alphanumeric, `_`, `-`). No regex quantifiers allowed.

### 2.2 Return Coalescing (Smart Selection)
When a return clause contains a choice list `[f1|f2|f3]`, the engine performs a **Coalesce** operation.
- **Logic**: Returns the first field in the list that exists and has a non-empty value in the record.
- **Protocol Adherence**: The returned field name in the RFC 2378 response will be the *actual* field name found (e.g., `-200:1:sn:1234567`).

## 3. Future RFC Enhancements (Deferred)

### 3.1 Response Aliasing (`as` keyword)
**Status**: Deferred to a future iteration.
- **Goal**: Support `return sn as serial` to override the field label in the response.
- **Reason**: This is an enhancement to the standard RFC 2378 response format and requires careful client/server coordination.

## 4. Roadmap for Implementation: Phase 18

#### Task 18.1: Choice-Based Selection Expansion
- **Engineering**: Update `protocol.rs` to parse `[f1|f2]` in selections. Update `Storage` trait to handle search expansion.

#### Task 18.2: Return Coalescing Logic
- **Engineering**: Implement the first-match return logic in `pharos-server` and `pharos-client`.

#### Task 18.3: Global Alias Mapping
- **Engineering**: Support `mapping.yaml` to define server-side synonyms that automatically expand into choice lists.
