<!--
/* ========================================================================
 * Project: pharos
 * Component: Documentation / Architecture
 * File: pharos-query-expansion-spec.md
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * Defines the technical grammar for "Choice-Based Selection" and "Return
 * Coalescing" in Pharos Queries.
 * * Traceability:
 * Related to Phase 18 and pharos-alias-mapping-plan.md.
 * ======================================================================== */
-->

# Pharos Query Expansion Specification

## 1. Grammatical Tokens
New types for the `protocol.rs` parser:

```rust
pub enum SelectionField {
    Single(String),
    Alternation(Vec<String>), // Parsed from [a|b|c]
}

pub struct ReturnBlock {
    pub choices: Vec<String>, // Parsed from [a|b|c]
}
```

## 2. Selection Alternation (`[f1|f2]=val`)
When an alternation is used in the selection clause:
1.  Parser validates each choice against a **Permittedlist**.
2.  The server expands the selection into a logical `OR` across the record's fields.
3.  Any record matching **any** of the fields in the alternation is returned.

## 3. Return Coalescing (`return [f1|f2]`)
When an alternation is used in the return clause:
1.  The server identifies the first field in the `choices` vector that exists for the record.
2.  The server returns the **original field name** and its value per RFC 2378.
    - Example: `return [serialNumber|sn]` → `-200:1:sn:1234567` (if `sn` was found).

## 4. DevSecOps Constraints
- **Complexity Limit**: Max 8 choices per bracket.
- **No Regex Quantifiers**: Any character in `.*+?{}` within the brackets will trigger a syntax error.
- **Recursion**: No nested brackets.
