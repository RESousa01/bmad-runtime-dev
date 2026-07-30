# LLM Review Checklist

## Per task

- [ ] Dependencies have complete fresh evidence.
- [ ] Named source identity/resource closure is exact.
- [ ] Failing evidence existed before behavioral implementation.
- [ ] Generated contracts are used; no duplicate transport authority.
- [ ] No skill/model/script/renderer authority bypass.
- [ ] Cancel/fail/retry/restart/resume paths are covered.
- [ ] Interactive/headless and Help/UI/catalog states agree.
- [ ] No unrelated refactor or hidden fallback.
- [ ] Every verification command ran freshly and passed.
- [ ] Every acceptance criterion points to evidence.
- [ ] Diff contains no secrets, generated noise, disabled tests, or weakened assertions.
- [ ] Task-result JSON is valid and input hashes match current checkout.

## Per phase

- [ ] Every phase task is complete; no skipped tasks.
- [ ] Generated output is clean/reproducible.
- [ ] Interface and migration changes are documented.
- [ ] Phase gate passes.
- [ ] Blockers are exact and prevent dependent/final claims.
- [ ] Context/run state is durable and current.

## Final

- [ ] 48/48 Method rows pass.
- [ ] 5/5 Builder production skills pass.
- [ ] 9/9 public and 1/1 internal Builder actions pass.
- [ ] Every required source member/resource is accounted.
- [ ] Rust/frontend/.NET/desktop/security/performance/migration/package/Azure/docs/pilot gates pass.
- [ ] No unresolved critical/high finding or unapproved residual risk.
- [ ] Traceability has zero missing required nodes/edges.
- [ ] Final scorecard is fresh and hash-bound.
- [ ] Independent adversarial review passes.
- [ ] Final `pnpm verify:bmad:100` passes after review.
