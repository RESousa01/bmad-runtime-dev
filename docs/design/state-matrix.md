# State matrix

**Task:** Phase 5, D1 — the governing contract `FE-27` is tested against.
**Companions:** [accessibility-contract.md](accessibility-contract.md),
[platform-shortcut-contract.md](platform-shortcut-contract.md).

Every surface owes the user five states. The audit's finding was never that
surfaces lacked error handling — it was that each one invented its own, so the
same host failure read as a different problem depending on which panel happened
to be open. `recovery_required` was written out in three places before
`app/hostFailure.ts` existed, and each spelling drew the line somewhere slightly
different.

## The five states

| State | Means | The failure it prevents |
|---|---|---|
| **happy** | The surface has what it needs and is showing it. | — |
| **empty** | Nothing to show, and nothing went wrong. | An empty list that reads as a broken one. |
| **loading** | An answer is on its way. | A blank surface the user cannot tell from an empty one. |
| **error** | The action failed; the app is otherwise fine. | A failure reported as an emptiness, so the user retries nothing. |
| **constraint** | The app worked as designed and the user met a limit. | A cap reported as a failure — or, worse, as a success. |

`constraint` is the one that gets dropped, and it is the one this app needs
most. A truncated file pick, a context cap, a retention ceiling: none of those
are errors and none of them are empty. Reporting a cap as an error tells the
user the app is broken; reporting it as a success tells them nothing was lost.

Two rules apply across every surface:

- **Loading never replaces stable context.** A refresh that blanks what the user
  was reading has taken something away to tell them it is fetching.
- **Empty states present exactly one next action.** More than one is a menu; the
  user reached an empty state because they do not yet know what to do.

## Surfaces

Each row is what that surface shows in each state. "—" means the state is
unreachable there, and the reason is given.

| Surface | happy | empty | loading | error | constraint |
|---|---|---|---|---|---|
| **Home** (`HomeView`) | Projects and recent tasks | Three distinct sentences: no host, no project, no tasks — see below | Host verifying | Host unavailable, with Retry host | — the index has no cap |
| **Task transcript** (`TaskWorkspace`) | Turns, tool calls paired with results | Starter intents | "Thinking…" beside the pending turn | Failure beside the question that failed, question preserved | Retention ceiling notice: oldest turns dropped |
| **Composer** | Draft, attachments, Send | Send disabled with the blocker named | Sending / importing | Failure keeps the draft; retry is pressing Send again | Attachment cap, per-file refusals |
| **Files** (`FilesInspector`) | Tree | Empty workspace | Directory expanding | Explorer unavailable, host's own message | Page limit as an in-tree "load more" row |
| **Review Center** (`ReviewPanel`) | Working tree, or the pending decision | No proposed changes | Reading status | Git or governed failure | — |
| **Model context** (`ContextWindow`) | Used / window, basis stated | No files attached | — measured with the send | — reported by the composer | Window unavailable: no meter drawn |
| **Skills** (`Bmad*Panel`) | Skills with availability verdicts | Catalogue empty | Loading library | Library failed to load | Unavailable and unknown verdicts per skill |
| **Activity** (`ActivityPanel`) | Journal entries with their evidence | "Nothing has happened yet" | "Reading activity…" | "Activity could not be read" | — |
| **Settings** | Sections | — always has sections | Provider status checking | Provider rejected the key | — |
| **Recovery review** | The prepared review and its decision | — prepared or absent | Preparing | Recovery failed | — |

Home's three empty sentences are separate because the causes nest: a host that
cannot answer is why there is no project, and no project is why there is no
task. The outermost unmet condition is the honest one to name. Saying "no tasks
yet — describe what you want above" while the composer is disabled for want of a
workspace points the user at a control that cannot accept them.

Activity distinguishes four caveats, not one: nothing happened, not read yet,
could not be read, and no journal kept for this workspace. The last is what a
workspace with governed edits disabled produces, and it is not evidence that
nothing happened.

## Host error codes

Every `LocalErrorCode` maps to a state and a recovery. The mapping lives in
`apps/desktop-ui/src/features/surface-state/surfaceState.ts` as a
`Record<LocalErrorCode, …>`, so a code added to the contract fails to compile
until someone decides what it means to a user. A `switch` with a `default` would
have absorbed it silently, which is how thirty codes come to be presented as
"Something went wrong".

The table below is generated from that mapping by
`tools/check-state-matrix.mjs`, and the same tool verifies it in the gate. The
compiler makes the mapping total; the tool makes this prose match it.

The host's own `retryable` flag overrides the table at render time. The table
says what kind of failure a code *is*; the flag says whether this particular
occurrence could go differently, and the host is the only thing that knows.
Offering a retry the host has already ruled out is a control that cannot
succeed.

<!-- BEGIN GENERATED: host-error-states -->

<!-- Generated by features/surface-state/stateMatrixDoc.test.ts.
     Do not edit between the markers; run the suite with UPDATE_STATE_MATRIX=1. -->

30 host error codes, all mapped.

| Code | State | Recovery | Note |
|---|---|---|---|
| `authentication_required` | error | reauthenticate |  |
| `bmad_projection_gap` | constraint | none | This skill is not projected into the local runtime. |
| `bmad_projection_unavailable` | error | retry |  |
| `conflict` | error | retry |  |
| `consent_already_consumed` | error | review_context |  |
| `consent_binding_mismatch` | error | review_context |  |
| `consent_expired` | constraint | review_context |  |
| `consent_required` | constraint | review_context |  |
| `context_drift` | constraint | review_context |  |
| `context_rejected` | error | review_context |  |
| `entitlement_unavailable` | constraint | open_settings |  |
| `expired` | error | retry |  |
| `feature_disabled` | constraint | none | This build does not include that feature. |
| `identity_unavailable` | error | reauthenticate |  |
| `integrity_failure` | error | recover_local_store |  |
| `internal` | error | retry | generic fallback |
| `invalid_model_output` | error | retry |  |
| `invalid_request` | error | none | This is a fault in Forge, not something you can change. |
| `not_found` | empty | none | It is no longer where Forge last saw it. |
| `reauthentication_required` | error | reauthenticate |  |
| `receipt_invalid` | error | recover_local_store |  |
| `recovery_required` | error | recover_local_store |  |
| `renderer_session_expired` | error | retry |  |
| `resource_limit` | constraint | review_context |  |
| `response_binding_mismatch` | error | retry |  |
| `support_plane_offline` | error | retry |  |
| `temporarily_unavailable` | error | retry |  |
| `tenant_mismatch` | error | open_settings |  |
| `transport_failed` | error | retry |  |
| `unauthorized` | error | reauthenticate |  |

Falling to the generic fallback, deliberately: `internal`.

<!-- END GENERATED: host-error-states -->

## Errors

An error state owes the user three things, and the third is the one usually
missing.

1. **Name the action that failed.** "Something went wrong" names nothing. "The
   message could not be sent" names it.
2. **Preserve their work.** A failed send keeps the draft and keeps the question
   standing in the transcript. Telling someone to try again while having
   discarded what they wrote is advice they cannot take.
3. **Offer a recovery that can succeed.** A retry on a deterministic refusal — a
   binary file, a sensitive path, a spent consent — is a control that changes
   nothing when pressed, and the user learns only that the app does not work.
   Where nothing can be done, say so and say why: `surfaceState`'s `none`
   recovery carries a required `because` for exactly this reason.
