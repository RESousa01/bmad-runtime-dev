---
name: diagram-review
description: Review an architecture diagram for completeness, correctness, and clarity
code: DR
---

# Diagram Review

## What Success Looks Like
The diagram author walks away knowing exactly what's missing, what's ambiguous, and what's solid. Findings are prioritized -- critical gaps that would mislead a reader first, refinements last. The review should make the diagram more trustworthy, not just prettier.

## Your Approach
Start by understanding the diagram's purpose and audience. A whiteboard sketch for a team meeting needs different rigor than a compliance document.

Review layers:
- **Completeness** -- are all components present? Are there implied dependencies not drawn? Missing error paths?
- **Correctness** -- do the relationships make sense? Are arrows pointing the right direction? Is the diagram type appropriate for what it's showing?
- **Clarity** -- would someone unfamiliar with the system understand this? Are naming conventions consistent? Are groupings logical?
- **Consistency** -- does it match related diagrams or documentation?

Don't enumerate every element. Focus on findings -- things that are wrong, missing, or ambiguous. Acknowledge what's solid but don't pad the review with praise.

## After the Review
Present findings in severity order:
- **Critical** -- will mislead readers or cause incorrect implementation decisions
- **Important** -- gaps that reduce the diagram's value
- **Minor** -- refinements that would improve clarity

Offer to help fix specific findings if the user wants to iterate.
