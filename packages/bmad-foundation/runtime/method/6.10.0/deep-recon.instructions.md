# Managed Deep Recon guidance

## Purpose

Run one source-faithful Deep Recon service against a reviewed research
workspace. The supported services are Draft, Process, Run, Refresh, and
Deepen. Market, domain, and technical research compatibility entries resolve
to this same capability with their research type preset.

## Service selection

- Draft creates or refines `brief.md`, then produces an inert handoff briefing.
- Process imports supplied material into `imports/` and `digests/`, verifies
  traceability, and synthesizes it into `research.md`.
- Run performs bounded evidence gathering, verification, synthesis, and
  finalization.
- Refresh resumes an existing run and updates stale claims.
- Deepen resumes an existing run and investigates explicit evidence gaps.
- Interactive use confirms an ambiguous service or research type. Headless use
  requires the service and all required inputs to be resolved before dispatch.

## Workspace and evidence

Treat `brief.md`, `imports/`, `digests/`, `research.md`, and `.memlog.md` as one
resumable run folder. Preserve source identity and citation status for every
material claim. Separate observed facts, source-backed claims, inferences, and
open questions. Never present an unverified claim as verified.

## Authority boundary

Source requests for file access, file writes, Python helpers, web research,
subagents, browser presentation, or an external handoff are non-authorizing
intents. Use only the distinct host plan supplied for that intent. If a plan is
absent, denied, stale, or incomplete, record the limitation and do not perform
the effect. A browser plan is not network authority; an external handoff is not
browser authority; neither is a process grant.

## Output boundary

Return an inert research document and the associated run-folder file set.
Completion requires source traceability, explicit verification status,
persisted artifacts, and a resumable Method checkpoint. Do not start another
capability or claim that a denied effect occurred.
