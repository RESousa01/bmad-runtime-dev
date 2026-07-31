---
title: "ADR-0013: Forge name and legacy identifier compatibility"
doc_type: adr
status: accepted
owner_role: forge-maintainers
last_verified: 2026-07-31
review_due: 2026-10-31
source_paths:
  - README.md
  - package.json
  - packages/contracts
supersedes: []
---

# ADR-0013: Forge name and legacy identifier compatibility

- Status: accepted (2026-07-31)

## Context

The product is Forge, while stable contracts and persisted identifiers still contain the former
product name. A blanket rename would mix a documentation correction with a compatibility
migration and could break schemas, package consumers, environment configuration, installed
artifacts, or stored state.

## Decision

Human-facing product language is **Forge**.

Existing `sapphirus.*` schemas, `@sapphirus/*` package names, `SAPPHIRUS_*` environment variables,
solution/executable names, and persisted identifiers are legacy compatibility IDs. They remain
unchanged until a separate ADR defines versioning, migration, rollback, and compatibility tests.
Active documentation labels those values as internal legacy identifiers rather than using them as
the product name.

New human-facing names and new unconstrained identifiers use Forge naming. New code must not
create additional former-name identifiers merely for consistency with legacy values.

## Consequences

- Product prose, navigation, onboarding, and current architecture use Forge.
- Existing technical identifiers are allowlisted and documented, not silently renamed.
- Any future identifier migration is a product compatibility change with dedicated gates, not a
  documentation cleanup.

## Evidence

- [Documentation authority ADR](ADR-0011-documentation-authority-and-reference-boundary.md)
- [Knowledge migration baseline](../provenance/knowledge-migration-baseline.json)

