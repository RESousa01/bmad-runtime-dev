# BMAD-G0 adversarial review

Date: 2026-07-14 (Europe/Brussels)
Reviewer: independent adversarial read-only lane
Verdict: **PASS / Looks good**

## Threats exercised

The review targeted generator/path substitution, global-tool shadowing,
transplanted external references, duplicate-key bypass, partial-language
coverage, junction/hard-link escapes, .NET startup hooks, profiler/runtime
selectors, and PowerShell module shadowing.

The final live exploit replay supplied mixed-case `DoTnEt_StArTuP_HoOkS` with
a real previously executable unsigned startup-hook DLL and a fresh marker.
Preflight returned the stable forbidden-inherited-environment error and the
marker was not created. A hostile `PSModulePath` could not replace the fixed
system Security/Utility modules and produced no execution marker.

All six native process call sites use a locked child-environment kind and an
immediate identity callback. Rust receives an empty environment; .NET/Corvus
receive only diagnostics-off values; Windows PowerShell receives only the
nonexistent module-search sentinel. No inherited `PATH`, home/profile, cache,
runtime-selection, profiler, dependency, or module-search state reaches these
children.

## Findings

- P0: none.
- P1: none. The inherited native-code execution blocker is closed.
- P2: exception-atomic rather than crash/power-loss-atomic generation; narrow
  same-user pathname TOCTOU. Both are explicitly bounded follow-ups.
- P3: none.

## Validation

- Native environment suite: 18/18 passed.
- Full focused Node suite: 39 passed, one platform capability skip.
- Cross-language verification twice: 76 passed plus the same capability skip
  per run, identical 90-file qualification and 622-file production outputs.
- Source verification twice, Rust fmt/clippy/70 tests, and C# 9 tests: passed.

The adversarial gate passes. The immutable upstream source-identity promotion
blocker remains a separate BMAD-00 provenance state and is not a BMAD-G0
generator defect.
