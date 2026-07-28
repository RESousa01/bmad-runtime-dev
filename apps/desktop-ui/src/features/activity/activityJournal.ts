/**
 * The Activity journal: what the app did, and the evidence for each claim.
 *
 * FE-24. Activity was a feed of status lines -- "Preparing a local review",
 * "Approving · nothing sent", "Run completed". None of them named a run, so
 * none of them could be checked against anything. A user opening Activity to
 * answer "did that change actually apply, and what proof is there" got
 * adjectives.
 *
 * This module holds the three decisions that turn host projections into a
 * record, and it holds them here rather than in the panel because each one is
 * a claim about truth, not about layout:
 *
 *   1. **What may become an entry.** An entry names the host-issued identity it
 *      is evidence of. A state the host reports without an identity -- a
 *      terminated guidance run, which carries a reason and nothing else -- is
 *      not an entry. It is reported separately, as an event the journal cannot
 *      attribute, because inventing a subject for it is exactly the failure
 *      this surface exists to avoid.
 *
 *   2. **What counts as a time.** `completedAt` and `updatedAt` arrive as
 *      `asSingleLineText(_, 64)` -- arbitrary text. The store writes RFC 3339,
 *      but the renderer contract does not require it, and `new Date(string)` is
 *      lenient enough to turn `"2"` into February 2001. So a value is a time
 *      only if it is *wholly* RFC 3339, names a day the calendar has, and
 *      parses; anything else is carried as the raw string it is. Ordering then
 *      follows: only entries with a real instant can be placed in time, and
 *      the rest are grouped apart rather than silently sorted to the bottom as
 *      though they were the oldest.
 *
 *   3. **What the journal knows about its own gaps.** "The journal is empty",
 *      "there is no journal" and "the journal could not be read" are three
 *      different claims, and the panel used to make the first whenever it had
 *      nothing to show. Worse, it only ever mentioned a failed read when the
 *      whole surface was empty: one guidance entry was enough to swallow the
 *      fact that the governed-change half was missing. The caveat is therefore
 *      computed from the read state alone and never from whether there is
 *      anything to show.
 *
 * What is deliberately *not* decided here is what a failed read means. That
 * belongs to `features/surface-state`, which already maps every host error code
 * to a state and a recovery and already lets the host's `retryable` flag
 * overrule the table. A journal that formed its own opinion would be the second
 * answer to a question that has one -- and the way this app previously came to
 * offer a retry for a failure that could never succeed.
 */

import type {
  BmadHelpCompletedRecommendationProjection,
  BmadRequestState,
} from "../../lib/bmadModelProjection";
import type {
  ChangesHistoryEntryProjection,
  ChangesHistoryProjection,
  ChangesOpenJournalProjection,
  LocalHostError,
} from "../../lib/hostClient";
import { presentHostError, type SurfaceRecovery } from "../surface-state/surfaceState";

/* -------------------------------------------------------------------------
 * Time
 * ---------------------------------------------------------------------- */

/**
 * When something happened, or the reason the journal cannot say.
 *
 * Three cases rather than `number | null`, because "the host recorded nothing"
 * and "the host recorded something that is not a time" are different facts and
 * the second one is a host defect the user should be able to see.
 */
export type JournalTime =
  /** A real instant. `label` names *which* time it is -- an unlabelled time on
   *  a record is ambiguous the moment an entry has more than one. */
  | { readonly kind: "recorded"; readonly label: string; readonly epochMs: number }
  /** The host sent a value where a time belongs and it is not one. */
  | { readonly kind: "unreadable"; readonly label: string; readonly raw: string }
  /** The projection carries no time at all. Carries no label: there is no time
   *  to name, and a label here would imply a field that does not exist. */
  | { readonly kind: "unrecorded" };

/**
 * RFC 3339, which is what `desktop-store`'s `canonical_now` writes.
 *
 * Anchored and complete, so nothing partial slips through to `Date.parse`.
 * `Date.parse` alone is not a validator: it accepts `"2"`, `"July"` and
 * `"1 2 3"` and returns a plausible instant for each. A journal that renders
 * one of those as a date is worse than one that renders no date at all.
 */
const RFC_3339 =
  /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?(?:Z|[+-]\d{2}:\d{2})$/u;

/**
 * How many days that month actually had.
 *
 * Needed because `Date.parse` does not reject a day the calendar does not
 * have; it rolls it forward. `2026-02-30T00:00:00Z` parses cleanly to
 * `2026-03-02`, `2026-02-29` to `2026-03-01`, `2026-04-31` to `2026-05-01`.
 * A journal that prints March 2 for a record the host wrote as February 30 has
 * invented a date, which is worse than admitting it cannot read one -- so the
 * day is checked against the month before the parse is trusted.
 *
 * The offset is irrelevant here: it moves the instant, not the calendar date
 * that was written down, so validity is a pure function of the written Y-M-D.
 */
function daysInMonth(year: number, month: number): number {
  if (month === 2) {
    const leapYear = year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
    return leapYear ? 29 : 28;
  }
  return month === 4 || month === 6 || month === 9 || month === 11 ? 30 : 31;
}

/** Reads a host-supplied time string without guessing at one. */
export function readInstant(label: string, raw: string): JournalTime {
  const text = raw.trim();
  if (text.length === 0) return { kind: "unrecorded" };
  if (!RFC_3339.test(text)) return { kind: "unreadable", label, raw };
  const year = Number(text.slice(0, 4));
  const month = Number(text.slice(5, 7));
  const day = Number(text.slice(8, 10));
  if (month < 1 || month > 12 || day < 1 || day > daysInMonth(year, month)) {
    return { kind: "unreadable", label, raw };
  }
  const epochMs = Date.parse(text);
  // Still needed after the calendar check, for the fields it does not cover:
  // `25:00:00` and a leap second `23:59:60` match the pattern and parse to NaN.
  if (!Number.isFinite(epochMs)) return { kind: "unreadable", label, raw };
  return { epochMs, kind: "recorded", label };
}

/**
 * One recorded instant, in the zone the host recorded it in.
 *
 * UTC rather than the reader's local time, for the same reason the byte counts
 * name their locale: this is evidence. Two people quoting the same receipt to
 * support have to read the same clock off it, and the host's log is in UTC.
 */
export function formatInstant(epochMs: number): string {
  return `${new Date(epochMs).toISOString().slice(0, 19).replace("T", " ")} UTC`;
}

/**
 * A time, or the reason there is not one, as a sentence.
 *
 * The two absent cases get words rather than a dash. A dash in a time column
 * is read as a time that is being withheld; these are a time that was never
 * recorded and a value the host sent that is not a time, and the second one is
 * a host defect worth showing rather than smoothing over.
 */
export function describeTime(time: JournalTime): string {
  switch (time.kind) {
    case "recorded":
      return `${time.label} ${formatInstant(time.epochMs)}`;
    case "unreadable":
      return `The host recorded ${time.label.toLowerCase()} as “${time.raw}”, which is not a time this app can read.`;
    case "unrecorded":
      return "No time recorded.";
  }
}

/* -------------------------------------------------------------------------
 * Entries
 * ---------------------------------------------------------------------- */

/**
 * One named piece of evidence on an entry.
 *
 * `form` is not styling. An `identifier` is a host-issued token the user may
 * have to quote back -- an execution id, a manifest hash, a receipt id -- and
 * it is shown verbatim, never abbreviated or case-folded. `text` is the app's
 * own prose about the host's value.
 */
export interface JournalFact {
  readonly label: string;
  readonly value: string;
  readonly form: "identifier" | "text";
}

/** The one thing an entry can offer to do. A journal is a record, not a menu. */
export type JournalAction =
  | { readonly kind: "undo"; readonly executionId: string }
  | { readonly kind: "review_recovery"; readonly journalId: string };

export interface JournalEntry {
  /** React key, and the identity this row is evidence about, namespaced by source. */
  readonly id: string;
  /** What happened, in the app's words. Never more than the evidence supports. */
  readonly headline: string;
  /**
   * The host-issued identity this entry is evidence of.
   *
   * Not optional. A row that cannot name its subject is a rumour about a run,
   * and such states never become entries -- see `unattributed`.
   */
  readonly subject: JournalFact;
  readonly time: JournalTime;
  readonly facts: readonly JournalFact[];
  readonly action: JournalAction | null;
  /** Why this entry offers no action, where the host gave a reason. */
  readonly note: string | null;
}

/* -------------------------------------------------------------------------
 * Sections
 * ---------------------------------------------------------------------- */

export type JournalSectionKey = "attention" | "recorded" | "undated";

export interface JournalSection {
  readonly key: JournalSectionKey;
  readonly title: string;
  /** What the grouping means -- and, for `undated`, why the order is not time. */
  readonly caption: string;
  readonly entries: readonly JournalEntry[];
}

/* -------------------------------------------------------------------------
 * What the journal cannot account for
 * ---------------------------------------------------------------------- */

export type JournalCaveatKey =
  | "nothing_yet"
  | "not_recorded"
  | "unread"
  | "reading"
  | "unreadable";

export interface JournalCaveat {
  readonly key: JournalCaveatKey;
  /**
   * The heading, used only when the caveat stands in place of the journal.
   *
   * Split from `body` because the same fact reads differently in the two
   * places it can appear: "No activity yet" is a heading over an empty panel
   * and a contradiction over a list of entries. `placement` decides which the
   * panel renders, and the panel never shows `title` above entries.
   */
  readonly title: string;
  /** The full sentence. Correct in both placements. */
  readonly body: string;
  /** The host's own reason for the failure, verbatim. Null when it gave none. */
  readonly reason: string | null;
  /**
   * The host's correlation id for the failed read.
   *
   * The only thing that lets a user's report be matched to a line in the host
   * log. Null whenever the failure did not carry one -- never a placeholder.
   */
  readonly correlationId: string | null;
  /**
   * What can be done about it, decided by `features/surface-state`.
   *
   * Taken from there rather than settled here, because this panel offering a
   * Refresh the host has already called final is the same defect that shipped
   * three times before `presentHostError` existed. Null means the shell handed
   * the journal a message but not the structured failure, so nobody has ruled
   * on this occurrence -- which is a different thing from ruling that nothing
   * can be done, and must not be shown as one.
   */
  readonly recovery: SurfaceRecovery | null;
  readonly placement: "empty" | "banner";
  readonly tone: "alert" | "neutral";
}

/**
 * One recovery, as something to read.
 *
 * The journal states the remedy rather than offering a control for it: it is a
 * record, and the actions that resolve these live on the surfaces that own
 * them. `none` is quoted straight from the table, which already carries the
 * host's own reason.
 */
export function describeRecovery(recovery: SurfaceRecovery): string {
  switch (recovery.kind) {
    case "retry":
      return "Refreshing may read it.";
    case "reauthenticate":
      return "Sign in again, then refresh.";
    case "regrant_workspace":
      return "Choose this workspace's folder again, then refresh.";
    case "review_context":
      return "Review the context selection before trying again.";
    case "recover_local_store":
      return "The local store has to be repaired before the journal can be read.";
    case "open_settings":
      return "Something in Settings has to change first.";
    case "none":
      return recovery.because;
  }
}

/* -------------------------------------------------------------------------
 * The journal
 * ---------------------------------------------------------------------- */

export interface ActivityJournal {
  /** Non-empty sections only, in the order they should be read. */
  readonly sections: readonly JournalSection[];
  /** Null exactly when the journal is a complete account and has entries. */
  readonly caveat: JournalCaveat | null;
  /**
   * Something the app knows happened but cannot attribute to a run the host
   * named. Deliberately not an entry: an entry names what it is evidence of.
   */
  readonly unattributed: string | null;
  readonly entryCount: number;
}

export type ChangesHistoryReadStatus =
  | "unread"
  | "loading"
  | "loaded"
  | "unavailable";

export interface ActivityJournalInput {
  readonly guidance: BmadRequestState;
  readonly history: ChangesHistoryProjection | null;
  /** False when the host keeps no execution journal for this workspace at all. */
  readonly historyRecorded: boolean;
  readonly historyStatus: ChangesHistoryReadStatus;
  /** The host's safe message for a failed read, where that is all the shell kept. */
  readonly historyError: string | null;
  /**
   * The failed read itself, where the shell kept it.
   *
   * The only source of a correlation id -- the one thing that lets a user's
   * report be matched to a line in the host log -- and the only thing that can
   * say whether *this* read could go differently, which is not something a
   * message string knows.
   */
  readonly historyFailure: LocalHostError | null;
}

/* -------------------------------------------------------------------------
 * Building entries
 * ---------------------------------------------------------------------- */

/** `recovery_required` -> `recovery required`. */
function phrase(identifier: string): string {
  return identifier.replaceAll("_", " ");
}

/** `recovery_required` -> `Recovery required`. */
function sentence(identifier: string): string {
  const words = phrase(identifier);
  const first = words.slice(0, 1);
  return first === "" ? identifier : first.toUpperCase() + words.slice(1);
}

/** Explicit locale: a journal read across machines must not group differently. */
function bytes(count: number): string {
  return `${count.toLocaleString("en-US")} bytes`;
}

function executionEntry(entry: ChangesHistoryEntryProjection): JournalEntry {
  return {
    action: entry.undoable
      ? { executionId: entry.executionId, kind: "undo" }
      : null,
    facts: [
      { form: "text", label: "Journal state", value: sentence(entry.journalState) },
    ],
    // The count is the host's own `file_count` for the recorded execution
    // result, so it is exact. Zero is stated as zero rather than rounded up
    // into "1 file" by an off-by-one in the plural branch.
    headline: entry.fileCount === 0
      ? "Governed change · no files recorded"
      : `Governed change · ${entry.fileCount.toLocaleString("en-US")} ${entry.fileCount === 1 ? "file" : "files"}`,
    id: `execution:${entry.executionId}`,
    note: null,
    subject: { form: "identifier", label: "Execution", value: entry.executionId },
    time: readInstant("Completed", entry.completedAt),
  };
}

/**
 * Why an unresolved journal can or cannot be acted on.
 *
 * The host decides this from the grant epoch and the journal state; the
 * renderer is told the outcome and must not narrate a cause it did not
 * receive.
 */
function availabilityNote(journal: ChangesOpenJournalProjection): string {
  switch (journal.recoveryAvailability) {
    case "quarantined":
      return "Select the exact workspace and governed-edits grant to review recovery.";
    case "manual_review":
      return "This journal requires manual review outside this recovery flow.";
    case "review_available":
      return "A bounded checkpoint review is available.";
  }
}

function openJournalEntry(journal: ChangesOpenJournalProjection): JournalEntry {
  return {
    action: journal.recoveryAvailability === "review_available"
      ? { journalId: journal.journalId, kind: "review_recovery" }
      : null,
    facts: [
      { form: "identifier", label: "Execution", value: journal.executionId },
    ],
    headline: sentence(journal.state),
    id: `journal:${journal.journalId}`,
    note: availabilityNote(journal),
    subject: { form: "identifier", label: "Journal", value: journal.journalId },
    time: readInstant("Updated", journal.updatedAt),
  };
}

function recommendationFacts(
  recommendation: BmadHelpCompletedRecommendationProjection,
): readonly JournalFact[] {
  if (recommendation.recommendationKind === "no_recommendation") {
    return [{
      form: "text",
      label: "Recommendation",
      // The reason code is the host's, restated rather than interpreted: the
      // renderer has no basis to say *why* the evidence was ambiguous.
      value: `None · ${phrase(recommendation.reasonCode)}`,
    }];
  }
  return [
    { form: "text", label: "Recommendation", value: recommendation.displayName },
    {
      form: "identifier",
      label: "Skill",
      value: recommendation.action === null
        ? `${recommendation.moduleCode}/${recommendation.skillName}`
        : `${recommendation.moduleCode}/${recommendation.skillName}#${recommendation.action}`,
    },
    // How much weight the host itself puts on the recommendation. Dropping it
    // would present a heuristic guess and an authoritative reading identically.
    {
      form: "text",
      label: "Evidence class",
      value: phrase(recommendation.evidenceClass),
    },
  ];
}

/**
 * The latest skill-guidance run, when it has produced evidence.
 *
 * In-flight states -- creating, review required, approving, submitting --
 * produce nothing. They are status about a request in progress, which is what
 * this surface stopped being. What survives is a decision that was taken
 * (`approved` carries its decision id and the manifest it is bound to) and a
 * run that reached an end (`completed` carries a verified receipt).
 */
function guidanceEntry(guidance: BmadRequestState): JournalEntry | null {
  switch (guidance.kind) {
    case "idle":
      if (guidance.run === null) return null;
      return {
        action: null,
        facts: [
          { form: "identifier", label: "Session", value: guidance.run.sessionId },
          { form: "identifier", label: "Lifecycle", value: guidance.run.lifecycle },
        ],
        headline: "Skill guidance run created",
        id: `guidance:${guidance.run.runId}`,
        note: null,
        subject: { form: "identifier", label: "Run", value: guidance.run.runId },
        time: { kind: "unrecorded" },
      };
    case "approved":
      return {
        action: null,
        facts: [
          { form: "identifier", label: "Decision", value: guidance.approval.decisionId },
          { form: "identifier", label: "Manifest", value: guidance.approval.manifestHash },
          { form: "identifier", label: "Session", value: guidance.run.sessionId },
        ],
        headline: "Guidance context approved for sending",
        id: `guidance:${guidance.run.runId}`,
        note: null,
        subject: { form: "identifier", label: "Run", value: guidance.run.runId },
        time: { kind: "unrecorded" },
      };
    case "completed":
      return {
        action: null,
        facts: [
          { form: "identifier", label: "Receipt", value: guidance.result.receipt.receiptId },
          { form: "identifier", label: "Session", value: guidance.result.sessionId },
          { form: "identifier", label: "Region", value: guidance.result.receipt.region },
          // The retention mode is quoted rather than translated. "Nothing was
          // stored" is a stronger sentence than the token licences.
          { form: "identifier", label: "Retention", value: guidance.result.receipt.retentionMode },
          { form: "text", label: "Sent", value: bytes(guidance.result.receipt.inputBytes) },
          { form: "text", label: "Returned", value: bytes(guidance.result.receipt.outputBytes) },
          ...recommendationFacts(guidance.result.recommendation),
        ],
        headline: "Skill guidance run completed",
        id: `guidance:${guidance.result.runId}`,
        note: null,
        subject: { form: "identifier", label: "Run", value: guidance.result.runId },
        // Taken without a `readInstant`-style guard on purpose. This one is a
        // number, and `bmadProtocol`'s decoder already put it through
        // `asPositiveSafeInteger` at the wire boundary, so a non-finite value
        // cannot reach here -- a second check would be unreachable code
        // dressed up as caution.
        time: {
          epochMs: guidance.result.receipt.completedAt,
          kind: "recorded",
          label: "Receipt completed",
        },
      };
    case "interrupted":
      return {
        action: null,
        facts: [
          { form: "identifier", label: "Session", value: guidance.result.sessionId },
          { form: "identifier", label: "Lifecycle", value: guidance.result.lifecycle },
        ],
        headline: "Skill guidance run interrupted",
        id: `guidance:${guidance.result.runId}`,
        note: null,
        subject: { form: "identifier", label: "Run", value: guidance.result.runId },
        time: { kind: "unrecorded" },
      };
    case "unavailable":
      // Only when the host named the run. The message alone identifies nothing.
      if (guidance.run === null) return null;
      return {
        action: null,
        facts: [
          { form: "text", label: "Reason", value: guidance.message },
          { form: "identifier", label: "Session", value: guidance.run.sessionId },
        ],
        headline: "Skill guidance run could not complete",
        id: `guidance:${guidance.run.runId}`,
        note: null,
        subject: { form: "identifier", label: "Run", value: guidance.run.runId },
        time: { kind: "unrecorded" },
      };
    case "creating":
    case "review_required":
    case "approving":
    case "submitting":
    case "terminal":
      return null;
  }
}

/**
 * A guidance outcome the host reported without naming the run it belongs to.
 *
 * `terminal` carries a reason and nothing else -- no run id, no session, no
 * time. Rendering it as a journal row would put a claim in the ledger with
 * nothing to check it against; dropping it silently would lose the fact that
 * a run ended. So it is stated as what it is: an event the journal cannot
 * attribute.
 */
function unattributedGuidance(guidance: BmadRequestState): string | null {
  if (guidance.kind === "terminal") {
    return `A skill-guidance run ended (${phrase(guidance.reason)}). The host did not name the run, so it is not recorded above.`;
  }
  if (guidance.kind === "unavailable" && guidance.run === null) {
    return `A skill-guidance run failed (${guidance.message}). The host did not name the run, so it is not recorded above.`;
  }
  return null;
}

/* -------------------------------------------------------------------------
 * Caveats
 * ---------------------------------------------------------------------- */

function caveatFor(
  input: ActivityJournalInput,
  entryCount: number,
): JournalCaveat | null {
  const placement = entryCount === 0 ? "empty" : "banner";
  if (!input.historyRecorded) {
    return {
      body: "Governed edits are not enabled for this workspace, so the host keeps no execution journal for it.",
      correlationId: null,
      key: "not_recorded",
      placement,
      reason: null,
      recovery: null,
      // Not "No activity yet". There being no journal is a fact about the
      // workspace's settings; it is not evidence that nothing happened, and
      // stating it as though it were is the same fabrication as printing an
      // estimate as a measurement.
      title: "No execution journal for this workspace",
      tone: "neutral",
    };
  }
  switch (input.historyStatus) {
    case "unread":
      return {
        body: "Forge has not asked the host for this workspace's execution journal. Refresh to read it.",
        correlationId: null,
        key: "unread",
        placement,
        reason: null,
        recovery: null,
        title: "Activity has not been read yet",
        tone: "neutral",
      };
    case "loading":
      return {
        body: "Asking the host for this workspace's execution journal.",
        correlationId: null,
        key: "reading",
        placement,
        reason: null,
        recovery: null,
        title: "Reading activity…",
        tone: "neutral",
      };
    case "unavailable": {
      const failure = input.historyFailure;
      return {
        body: "Forge could not read this workspace's execution journal, so it cannot say whether anything happened.",
        correlationId: failure?.correlationId ?? null,
        key: "unreadable",
        placement,
        // The structured failure wins where there is one: it is the same
        // sentence the shell would have flattened into `historyError`, taken
        // from the value that also carries the id and the retry verdict, so
        // the three cannot come from different failures.
        reason: failure === null ? input.historyError : failure.safeMessage,
        recovery: failure === null ? null : presentHostError(failure).recovery,
        title: "Activity could not be read",
        tone: "alert",
      };
    }
    case "loaded":
      // The only sentence in this module that claims something about the
      // workspace rather than about the read, and the only one that requires
      // the read to have succeeded.
      return entryCount > 0 ? null : {
        body: "Forge read this workspace's execution journal and it is empty. Governed changes and skill-guidance runs appear here as they happen.",
        correlationId: null,
        key: "nothing_yet",
        placement: "empty",
        reason: null,
        recovery: null,
        title: "Nothing has happened yet",
        tone: "neutral",
      };
  }
}

/* -------------------------------------------------------------------------
 * Assembly
 * ---------------------------------------------------------------------- */

function section(
  key: JournalSectionKey,
  title: string,
  caption: string,
  entries: readonly JournalEntry[],
): readonly JournalSection[] {
  return entries.length === 0 ? [] : [{ caption, entries, key, title }];
}

export function buildActivityJournal(
  input: ActivityJournalInput,
): ActivityJournal {
  const attention = (input.history?.openJournals ?? []).map(openJournalEntry);

  // Collected in one fixed order -- governed executions as the host returned
  // them, then the guidance run -- so that entries the sort cannot separate
  // keep a deterministic position instead of one that depends on the sort.
  const rest: JournalEntry[] = (input.history?.entries ?? []).map(executionEntry);
  const guidance = guidanceEntry(input.guidance);
  if (guidance !== null) rest.push(guidance);

  const dated: { readonly entry: JournalEntry; readonly epochMs: number }[] = [];
  const undated: JournalEntry[] = [];
  for (const entry of rest) {
    if (entry.time.kind === "recorded") {
      dated.push({ entry, epochMs: entry.time.epochMs });
    } else {
      undated.push(entry);
    }
  }
  // Stable by specification, so equal instants keep collection order rather
  // than an order the comparator invented.
  dated.sort((left, right) => right.epochMs - left.epochMs);

  const sections = [
    ...section(
      "attention",
      "Needs attention",
      "These executions did not finish. The host is holding each one until it is resolved.",
      attention,
    ),
    ...section(
      "recorded",
      "Recorded",
      "Newest first, by the instant the host recorded.",
      dated.map(({ entry }) => entry),
    ),
    ...section(
      "undated",
      "Recorded without a usable time",
      "The host did not record a time this app can read for these, so they are in the order the host returned them — not in time order.",
      undated,
    ),
  ];
  const entryCount = attention.length + rest.length;

  return {
    caveat: caveatFor(input, entryCount),
    entryCount,
    sections,
    unattributed: unattributedGuidance(input.guidance),
  };
}
