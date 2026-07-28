import { describe, expect, it } from "vitest";

import type { BmadRequestState } from "../../lib/bmadModelProjection";
import type { BmadHelpRunCreatedProjection } from "../../lib/bmadProjection";
import type {
  ChangesHistoryEntryProjection,
  ChangesHistoryProjection,
  ChangesOpenJournalProjection,
  LocalHostError,
} from "../../lib/hostClient";
import {
  buildActivityJournal,
  describeRecovery,
  describeTime,
  readInstant,
  type ActivityJournal,
  type ActivityJournalInput,
  type JournalEntry,
  type JournalSectionKey,
} from "./activityJournal";

/* ---------------------------------------------------------------- fixtures */

const run: BmadHelpRunCreatedProjection = {
  schemaVersion: "bmad-help-run.v1",
  runKind: "bmad_help",
  lifecycle: "created_unbound",
  workspaceId: "workspace_1",
  runId: "run_1",
  sessionId: "session_1",
  currentIntent: "Review architecture readiness",
  runnable: false,
  completionClaimed: false,
  recommendation: {
    schemaVersion: "bmad-help-recommendation.v1",
    displayName: "BMad Help",
    moduleCode: "core",
    skillName: "bmad-help",
    action: null,
    confidence: "unknown",
    source: {
      sourceKind: "sealed_foundation",
      packageName: "bmad-method",
      packageVersion: "6.10.0",
    },
    reason: "The sealed Help capability matches this intent.",
    requiredGuidance: true,
    expectedArtifacts: [],
    availability: "capability_disabled",
    blockerCodes: ["bmad_capability_disabled"],
    completionClaimed: false,
  },
};

const completed: Extract<BmadRequestState, { kind: "completed" }> = {
  kind: "completed",
  result: {
    schemaVersion: "bmad-help-completed.v1",
    runKind: "bmad_help",
    lifecycle: "completed",
    workspaceId: "workspace_1",
    runId: "run_completed",
    sessionId: "session_completed",
    runnable: false,
    completionClaimed: true,
    recommendation: {
      recommendationKind: "recommended_capability",
      displayName: "Create Architecture",
      moduleCode: "bmm",
      skillName: "architecture-create",
      action: "create",
      evidenceClass: "authoritative",
      guidanceRequired: false,
      rationaleSummary: "Recommended next step",
      createdAt: 1_784_712_600_000,
    },
    receipt: {
      schemaVersion: "bmad-model-receipt-summary.v1",
      receiptId: "receipt_1",
      status: "succeeded",
      retentionMode: "transient_no_store",
      region: "localdev",
      inputBytes: 12_345,
      outputBytes: 256,
      startedAt: 1_784_712_599_000,
      completedAt: 1_784_712_600_000,
    },
  },
};

function execution(
  overrides: Partial<ChangesHistoryEntryProjection> = {},
): ChangesHistoryEntryProjection {
  return {
    executionId: "execution_1",
    journalState: "completed",
    fileCount: 2,
    completedAt: "2026-07-18T09:30:00Z",
    undoable: false,
    ...overrides,
  };
}

function openJournal(
  overrides: Partial<ChangesOpenJournalProjection> = {},
): ChangesOpenJournalProjection {
  return {
    journalId: "journal_1",
    executionId: "execution_9",
    state: "recovery_required",
    updatedAt: "2026-07-18T10:00:00Z",
    recoveryAvailability: "review_available",
    ...overrides,
  };
}

function history(
  entries: readonly ChangesHistoryEntryProjection[] = [],
  openJournals: readonly ChangesOpenJournalProjection[] = [],
): ChangesHistoryProjection {
  return {
    workspaceId: "workspace_1",
    entries: [...entries],
    openJournals: [...openJournals],
  };
}

function input(overrides: Partial<ActivityJournalInput> = {}): ActivityJournalInput {
  return {
    guidance: { kind: "idle", run: null },
    history: history(),
    historyRecorded: true,
    historyStatus: "loaded",
    historyError: null,
    historyFailure: null,
    ...overrides,
  };
}

function hostError(overrides: Partial<LocalHostError> = {}): LocalHostError {
  return {
    code: "transport_failed",
    safeMessage: "The host could not read the journal.",
    retryable: true,
    correlationId: "corr_1",
    ...overrides,
  };
}

function build(overrides: Partial<ActivityJournalInput> = {}): ActivityJournal {
  return buildActivityJournal(input(overrides));
}

function sectionEntries(
  journal: ActivityJournal,
  key: JournalSectionKey,
): readonly JournalEntry[] {
  return journal.sections.find((section) => section.key === key)?.entries ?? [];
}

function idsIn(journal: ActivityJournal, key: JournalSectionKey): readonly string[] {
  return sectionEntries(journal, key).map((entry) => entry.id);
}

function factValue(entry: JournalEntry, label: string): string | null {
  return entry.facts.find((fact) => fact.label === label)?.value ?? null;
}

/* -------------------------------------------------------------------- time */

describe("reading a host time", () => {
  it("reads an RFC 3339 instant and keeps which time it is", () => {
    expect(readInstant("Completed", "2026-07-18T09:30:00Z")).toEqual({
      epochMs: Date.UTC(2026, 6, 18, 9, 30, 0),
      kind: "recorded",
      label: "Completed",
    });
  });

  it("honours the host's offset instead of the reader's own clock", () => {
    // Same instant, written from a machine two hours ahead. A journal that
    // dropped the offset would place the two entries ninety minutes apart.
    expect(readInstant("Completed", "2026-07-18T11:30:00+02:00")).toEqual({
      epochMs: Date.UTC(2026, 6, 18, 9, 30, 0),
      kind: "recorded",
      label: "Completed",
    });
  });

  it("keeps sub-second precision", () => {
    expect(readInstant("Completed", "2026-07-18T09:30:00.250Z")).toEqual({
      epochMs: Date.UTC(2026, 6, 18, 9, 30, 0) + 250,
      kind: "recorded",
      label: "Completed",
    });
  });

  it("reports an absent time as absent rather than as now", () => {
    // `toEqual` on the whole value, not `.kind`: the point is that there is no
    // `epochMs` on it at all for a renderer to fall back on.
    expect(readInstant("Completed", "")).toEqual({ kind: "unrecorded" });
    expect(readInstant("Completed", "   ")).toEqual({ kind: "unrecorded" });
  });

  it("refuses the strings Date.parse would happily guess at", () => {
    // Every one of these parses to a plausible instant. `"2"` is February 2001.
    for (const raw of ["2", "July", "1 2 3", "2026", "Jul 18 2026"]) {
      expect(readInstant("Completed", raw)).toEqual({
        kind: "unreadable",
        label: "Completed",
        raw,
      });
    }
  });

  it("refuses a time with no zone, which reads differently on every machine", () => {
    expect(readInstant("Completed", "2026-07-18T09:30:00").kind).toBe("unreadable");
  });

  it("refuses a date with no time", () => {
    expect(readInstant("Completed", "2026-07-18").kind).toBe("unreadable");
  });

  it("refuses anything wrapped around a valid instant", () => {
    expect(readInstant("Completed", "at 2026-07-18T09:30:00Z").kind).toBe("unreadable");
    expect(readInstant("Completed", "2026-07-18T09:30:00Z (utc)").kind).toBe("unreadable");
  });

  /**
   * Why the pattern is anchored, rather than merely looked for.
   *
   * `Date.parse`'s fallback parser -- the one outside the spec's ISO grammar,
   * whose behaviour is implementation-defined -- ignores parenthesised text.
   * So a value that *contains* an instant can parse cleanly to an instant that
   * has nothing to do with it. The string below is what `Date.prototype
   * .toString` emits, with an ISO timestamp where the zone name belongs: it
   * says July 2026 in its parentheses and parses to midnight on 1 January
   * 2000, twenty-six years earlier.
   *
   * Neither downstream guard saves it. The calendar check would be reading
   * characters 5-9 of "Sat Jan 01 2000…" -- `"an"` and `"01"`, a NaN month and
   * a plausible day -- and the finite check would be reading a perfectly
   * finite number. Anchoring is the only thing standing between the host's
   * value and the wrong century.
   */
  it("requires the whole value to be an instant, not merely to contain one", () => {
    const raw = "Sat Jan 01 2000 00:00:00 GMT (2026-07-18T09:30:00Z)";
    expect(Date.parse(raw)).toBe(Date.UTC(2000, 0, 1));
    expect(readInstant("Completed", raw)).toEqual({
      kind: "unreadable",
      label: "Completed",
      raw,
    });
  });

  /**
   * The one `Date.parse` gets wrong in the direction that matters. It does not
   * reject a day the calendar does not have -- it rolls it forward, silently.
   * February 30 becomes March 2, February 29 in a common year becomes March 1,
   * April 31 becomes May 1. Every one of those is the journal printing a date
   * no host ever wrote.
   */
  it("refuses a calendar date that does not exist instead of rolling it forward", () => {
    expect(Date.parse("2026-02-30T00:00:00Z")).toBe(Date.UTC(2026, 2, 2));
    expect(readInstant("Completed", "2026-02-30T00:00:00Z")).toEqual({
      kind: "unreadable",
      label: "Completed",
      raw: "2026-02-30T00:00:00Z",
    });
    for (const raw of [
      "2026-02-29T00:00:00Z",
      "2026-04-31T00:00:00Z",
      "2026-06-31T00:00:00Z",
      "2026-09-31T00:00:00Z",
      "2026-11-31T00:00:00Z",
      "1900-02-29T00:00:00Z",
    ]) {
      expect(readInstant("Completed", raw).kind).toBe("unreadable");
    }
  });

  it("still reads the dates the calendar does have", () => {
    for (const [raw, expected] of [
      ["2024-02-29T00:00:00Z", Date.UTC(2024, 1, 29)],
      ["2000-02-29T00:00:00Z", Date.UTC(2000, 1, 29)],
      ["2026-01-31T00:00:00Z", Date.UTC(2026, 0, 31)],
      ["2026-12-31T23:59:59Z", Date.UTC(2026, 11, 31, 23, 59, 59)],
    ] as const) {
      expect(readInstant("Completed", raw)).toEqual({
        epochMs: expected,
        kind: "recorded",
        label: "Completed",
      });
    }
  });

  it("refuses a clock reading the calendar check cannot see", () => {
    // The day is valid and the pattern matches; only `Date.parse` catches
    // these, which is why the finite check survives alongside the calendar one.
    expect(readInstant("Completed", "2026-07-18T25:00:00Z").kind).toBe("unreadable");
    expect(readInstant("Completed", "2026-07-18T23:59:60Z").kind).toBe("unreadable");
  });

  it("hands the host's own value back when it cannot read it", () => {
    // Not normalised, not trimmed. If a user has to report this, they have to
    // be reporting what the host actually sent.
    expect(readInstant("Updated", "  yesterday  ")).toEqual({
      kind: "unreadable",
      label: "Updated",
      raw: "  yesterday  ",
    });
  });
});

describe("saying a time out loud", () => {
  it("prints a recorded instant in the host's zone, not the reader's", () => {
    // UTC, and labelled UTC. Two people quoting this receipt to support have
    // to be quoting the same clock.
    expect(describeTime({
      epochMs: Date.UTC(2026, 6, 18, 9, 30, 0),
      kind: "recorded",
      label: "Completed",
    })).toBe("Completed 2026-07-18 09:30:00 UTC");
  });

  it("says there is no time rather than drawing a dash", () => {
    // A dash in a time slot reads as a time being withheld.
    expect(describeTime({ kind: "unrecorded" })).toBe("No time recorded.");
  });

  it("shows the unreadable value instead of hiding the host's defect", () => {
    expect(describeTime({ kind: "unreadable", label: "Completed", raw: "soon" }))
      .toBe("The host recorded completed as “soon”, which is not a time this app can read.");
  });
});

/* ----------------------------------------------------------------- entries */

describe("what may become an entry", () => {
  it("gives every entry a host-issued subject", () => {
    // The rule the whole surface exists for: a row that cannot name what it is
    // evidence of is a rumour about a run.
    const journal = build({
      guidance: completed,
      history: history(
        [execution(), execution({ executionId: "execution_2", completedAt: "" })],
        [openJournal()],
      ),
    });
    const entries = journal.sections.flatMap((section) => [...section.entries]);
    expect(entries).toHaveLength(4);
    for (const entry of entries) {
      expect(entry.subject.form).toBe("identifier");
      expect(entry.subject.value.length).toBeGreaterThan(0);
      expect(entry.subject.label.length).toBeGreaterThan(0);
    }
  });

  it("records a governed execution against its execution id, verbatim", () => {
    const journal = build({
      history: history([execution({ executionId: "execution_MiXeD_01J" })]),
    });
    expect(sectionEntries(journal, "recorded")[0]?.subject).toEqual({
      form: "identifier",
      label: "Execution",
      value: "execution_MiXeD_01J",
    });
  });

  it("namespaces entry ids so two sources cannot collide", () => {
    // A host that issues `abc` as both an execution id and a run id must not
    // produce one row that overwrites the other.
    const journal = build({
      guidance: { kind: "idle", run: { ...run, runId: "abc" } },
      history: history([execution({ executionId: "abc" })], [openJournal({ journalId: "abc" })]),
    });
    const ids = journal.sections.flatMap((section) => section.entries.map((e) => e.id));
    expect(ids).toEqual(["journal:abc", "execution:abc", "guidance:abc"]);
  });

  it("counts files exactly, and says zero as zero", () => {
    const counts = [0, 1, 2, 1234].map((fileCount) =>
      sectionEntries(
        build({ history: history([execution({ fileCount })]) }),
        "recorded",
      )[0]?.headline);
    expect(counts).toEqual([
      "Governed change · no files recorded",
      "Governed change · 1 file",
      "Governed change · 2 files",
      "Governed change · 1,234 files",
    ]);
  });

  it("offers undo only where the host said the execution is undoable", () => {
    const undoable = build({ history: history([execution({ undoable: true })]) });
    expect(sectionEntries(undoable, "recorded")[0]?.action)
      .toEqual({ executionId: "execution_1", kind: "undo" });
    const fixed = build({ history: history([execution({ undoable: false })]) });
    expect(sectionEntries(fixed, "recorded")[0]?.action).toBeNull();
  });

  it("offers recovery review only where the host said it is available", () => {
    const journal = build({
      history: history([], [
        openJournal({ journalId: "j_open", recoveryAvailability: "review_available" }),
        openJournal({ journalId: "j_quarantined", recoveryAvailability: "quarantined" }),
        openJournal({ journalId: "j_manual", recoveryAvailability: "manual_review" }),
      ]),
    });
    const actions = sectionEntries(journal, "attention").map((entry) => entry.action);
    expect(actions).toEqual([
      { journalId: "j_open", kind: "review_recovery" },
      null,
      null,
    ]);
  });

  it("says why a journal it cannot act on cannot be acted on", () => {
    // Offering no action and offering no explanation are different failures.
    const journal = build({
      history: history([], [
        openJournal({ journalId: "j_quarantined", recoveryAvailability: "quarantined" }),
        openJournal({ journalId: "j_manual", recoveryAvailability: "manual_review" }),
      ]),
    });
    for (const entry of sectionEntries(journal, "attention")) {
      expect(entry.action).toBeNull();
      expect(entry.note ?? "").not.toBe("");
    }
  });

  it("never turns a request that is still in flight into a record", () => {
    // Status about work in progress is exactly what this surface stopped
    // being. None of these has produced evidence of anything yet.
    const inFlight: BmadRequestState["kind"][] = [
      "creating",
      "review_required",
      "approving",
      "submitting",
    ];
    for (const kind of inFlight) {
      const guidance = { ...completed, kind } as unknown as BmadRequestState;
      expect(build({ guidance }).entryCount).toBe(0);
    }
  });

  it("records an approved decision against the manifest it is bound to", () => {
    const guidance = {
      kind: "approved",
      run: { workspaceId: "workspace_1", runId: "run_a", sessionId: "session_a" },
      runProjection: null,
      review: null,
      authority: null,
      approval: {
        manifestHash: `sha256:${"a".repeat(64)}`,
        decisionId: "decision_a",
        expiresAt: 1,
        sendEligible: true,
      },
    } as unknown as BmadRequestState;
    const entry = sectionEntries(build({ guidance }), "undated")[0];
    expect(entry?.subject.value).toBe("run_a");
    expect(factValue(entry!, "Decision")).toBe("decision_a");
    expect(factValue(entry!, "Manifest")).toBe(`sha256:${"a".repeat(64)}`);
  });

  it("records a completed run against the receipt that proves it", () => {
    const entry = sectionEntries(build({ guidance: completed }), "recorded")[0];
    expect(entry?.subject).toEqual({
      form: "identifier",
      label: "Run",
      value: "run_completed",
    });
    expect(factValue(entry!, "Receipt")).toBe("receipt_1");
    expect(factValue(entry!, "Region")).toBe("localdev");
    // Quoted, not translated. "Nothing was stored" is a stronger claim than
    // the token licenses.
    expect(factValue(entry!, "Retention")).toBe("transient_no_store");
    expect(factValue(entry!, "Sent")).toBe("12,345 bytes");
    expect(factValue(entry!, "Returned")).toBe("256 bytes");
    expect(entry?.time).toEqual({
      epochMs: 1_784_712_600_000,
      kind: "recorded",
      label: "Receipt completed",
    });
  });

  it("carries how much weight the host puts on its own recommendation", () => {
    // Dropping this would present a heuristic guess and an authoritative
    // reading identically.
    const entry = sectionEntries(build({ guidance: completed }), "recorded")[0];
    expect(factValue(entry!, "Recommendation")).toBe("Create Architecture");
    expect(factValue(entry!, "Skill")).toBe("bmm/architecture-create#create");
    expect(factValue(entry!, "Evidence class")).toBe("authoritative");
  });

  it("restates the host's no-recommendation reason instead of interpreting it", () => {
    const guidance: BmadRequestState = {
      kind: "completed",
      result: {
        ...completed.result,
        recommendation: {
          recommendationKind: "no_recommendation",
          reasonCode: "completion_evidence_ambiguous",
          createdAt: 1,
        },
      },
    };
    const entry = sectionEntries(build({ guidance }), "recorded")[0];
    expect(factValue(entry!, "Recommendation"))
      .toBe("None · completion evidence ambiguous");
    // The renderer has no basis to say *why* the evidence was ambiguous, so it
    // must not have invented a skill to blame.
    expect(factValue(entry!, "Skill")).toBeNull();
  });

  it("records an interrupted run under the run the host named", () => {
    const entry = sectionEntries(
      build({ guidance: { kind: "interrupted", result: run } }),
      "undated",
    )[0];
    expect(entry?.headline).toBe("Skill guidance run interrupted");
    expect(entry?.subject.value).toBe("run_1");
    expect(factValue(entry!, "Session")).toBe("session_1");
  });

  it("records a failed run when the host named it", () => {
    const journal = build({
      guidance: { kind: "unavailable", message: "The model refused.", run },
    });
    const entry = sectionEntries(journal, "undated")[0];
    expect(entry?.subject.value).toBe("run_1");
    expect(factValue(entry!, "Reason")).toBe("The model refused.");
    expect(journal.unattributed).toBeNull();
  });

  it("keeps a run the host could not name out of the ledger, and says so", () => {
    // `terminal` carries a reason and nothing else. A row for it would be a
    // claim in the ledger with nothing to check it against.
    const journal = build({ guidance: { kind: "terminal", reason: "consent_expired" } });
    expect(journal.entryCount).toBe(0);
    expect(journal.sections).toEqual([]);
    expect(journal.unattributed)
      .toBe("A skill-guidance run ended (consent expired). The host did not name the run, so it is not recorded above.");
  });

  it("does the same for a failure the host reported without a run", () => {
    const journal = build({
      guidance: { kind: "unavailable", message: "The provider is offline.", run: null },
    });
    expect(journal.entryCount).toBe(0);
    expect(journal.unattributed)
      .toBe("A skill-guidance run failed (The provider is offline.). The host did not name the run, so it is not recorded above.");
  });

  it("reports nothing unattributed when everything is attributed", () => {
    expect(build({ guidance: completed }).unattributed).toBeNull();
    expect(build({ guidance: { kind: "idle", run: null } }).unattributed).toBeNull();
  });

  it("counts an idle run only once the host has created one", () => {
    expect(build({ guidance: { kind: "idle", run: null } }).entryCount).toBe(0);
    expect(build({ guidance: { kind: "idle", run } }).entryCount).toBe(1);
  });
});

/* ------------------------------------------------------------------- order */

describe("order", () => {
  it("puts the newest recorded instant first", () => {
    const journal = build({
      history: history([
        execution({ executionId: "oldest", completedAt: "2026-07-16T00:00:00Z" }),
        execution({ executionId: "newest", completedAt: "2026-07-18T00:00:00Z" }),
        execution({ executionId: "middle", completedAt: "2026-07-17T00:00:00Z" }),
      ]),
    });
    expect(idsIn(journal, "recorded"))
      .toEqual(["execution:newest", "execution:middle", "execution:oldest"]);
  });

  it("keeps equal instants in the order the host returned them", () => {
    // Exact array, not a pairwise ordering: an ordering assertion passes when
    // one of the two entries is missing entirely.
    const at = "2026-07-18T00:00:00Z";
    const journal = build({
      history: history([
        execution({ executionId: "first", completedAt: at }),
        execution({ executionId: "second", completedAt: at }),
        execution({ executionId: "third", completedAt: at }),
      ]),
    });
    expect(idsIn(journal, "recorded"))
      .toEqual(["execution:first", "execution:second", "execution:third"]);
  });

  it("groups entries with no usable time apart instead of sorting them oldest", () => {
    const journal = build({
      history: history([
        execution({ executionId: "dated", completedAt: "2026-07-18T00:00:00Z" }),
        execution({ executionId: "blank", completedAt: "" }),
        execution({ executionId: "garbled", completedAt: "last Tuesday" }),
      ]),
    });
    expect(idsIn(journal, "recorded")).toEqual(["execution:dated"]);
    expect(idsIn(journal, "undated"))
      .toEqual(["execution:blank", "execution:garbled"]);
  });

  it("says out loud that the undated group is not in time order", () => {
    const journal = build({ history: history([execution({ completedAt: "" })]) });
    const undated = journal.sections.find((section) => section.key === "undated");
    expect(undated?.caption).toContain("not in time order");
  });

  it("holds open journals in their own section whatever their time", () => {
    // An open journal is not a thing that happened; it is a thing that has not
    // finished. Sorting it into the record by its `updatedAt` would file an
    // unresolved execution as history.
    const journal = build({
      history: history(
        [execution({ executionId: "done", completedAt: "2026-07-18T00:00:00Z" })],
        [openJournal({ journalId: "j_newer", updatedAt: "2026-07-19T00:00:00Z" })],
      ),
    });
    expect(idsIn(journal, "attention")).toEqual(["journal:j_newer"]);
    expect(idsIn(journal, "recorded")).toEqual(["execution:done"]);
  });

  it("reads attention first, then the record, then the undated", () => {
    const journal = build({
      guidance: { kind: "idle", run },
      history: history([execution()], [openJournal()]),
    });
    expect(journal.sections.map((section) => section.key))
      .toEqual(["attention", "recorded", "undated"]);
  });

  it("emits no section it has no entries for", () => {
    expect(build().sections).toEqual([]);
    expect(build({ history: history([execution()]) }).sections.map((s) => s.key))
      .toEqual(["recorded"]);
  });

  it("counts every entry it emitted, and nothing it did not", () => {
    const journal = build({
      guidance: { kind: "idle", run },
      history: history([execution(), execution({ executionId: "e2", completedAt: "" })], [openJournal()]),
    });
    const rendered = journal.sections.reduce((total, s) => total + s.entries.length, 0);
    expect(journal.entryCount).toBe(4);
    expect(rendered).toBe(4);
  });
});

/* ----------------------------------------------------------------- caveats */

describe("what the journal says about its own gaps", () => {
  it("gives the empty, unread and unreadable states three different sentences", () => {
    const empty = build().caveat;
    const unread = build({ historyStatus: "unread", history: null }).caveat;
    const unreadable = build({
      historyStatus: "unavailable",
      history: null,
      historyError: "The host could not read the journal.",
    }).caveat;

    expect(empty?.title).toBe("Nothing has happened yet");
    expect(unread?.title).toBe("Activity has not been read yet");
    expect(unreadable?.title).toBe("Activity could not be read");
    expect(new Set([empty?.body, unread?.body, unreadable?.body]).size).toBe(3);
  });

  it("does not claim an empty workspace when the host keeps no journal for it", () => {
    // Governed edits being off is a fact about settings. It is not evidence
    // that nothing happened, and titling it "No activity yet" says it is.
    const caveat = build({ historyRecorded: false }).caveat;
    expect(caveat?.key).toBe("not_recorded");
    expect(caveat?.title).toBe("No execution journal for this workspace");
    expect(caveat?.title).not.toBe("Nothing has happened yet");
  });

  it("reports a missing journal whether or not the read state looks fine", () => {
    for (const historyStatus of ["unread", "loading", "loaded", "unavailable"] as const) {
      expect(build({ historyRecorded: false, historyStatus }).caveat?.key)
        .toBe("not_recorded");
    }
  });

  it("still reports a failed read when there is something to show", () => {
    // The bug this replaces: one guidance entry was enough to swallow the fact
    // that the governed-change half of the journal was missing entirely.
    const journal = build({
      guidance: completed,
      history: null,
      historyStatus: "unavailable",
      historyError: "The host could not read the journal.",
    });
    expect(journal.entryCount).toBe(1);
    expect(journal.caveat?.key).toBe("unreadable");
    expect(journal.caveat?.placement).toBe("banner");
    expect(journal.caveat?.tone).toBe("alert");
  });

  it("stands in place of the journal only when there is nothing to stand in for", () => {
    expect(build({ historyStatus: "unread" }).caveat?.placement).toBe("empty");
    expect(build({ historyStatus: "unread", guidance: completed }).caveat?.placement)
      .toBe("banner");
  });

  it("says nothing happened only after a read that succeeded", () => {
    expect(build({ historyStatus: "loaded" }).caveat?.key).toBe("nothing_yet");
    for (const historyStatus of ["unread", "loading", "unavailable"] as const) {
      expect(build({ historyStatus }).caveat?.key).not.toBe("nothing_yet");
    }
  });

  it("falls silent once the journal is a complete account with entries in it", () => {
    expect(build({ history: history([execution()]) }).caveat).toBeNull();
  });

  it("carries the host's reason for the failed read, verbatim", () => {
    expect(build({
      historyStatus: "unavailable",
      historyError: "Journal file is locked by another process.",
    }).caveat?.reason).toBe("Journal file is locked by another process.");
  });

  it("carries a correlation id only when the failure came with one", () => {
    // The only thing that lets a user's report be matched to a host log line,
    // and never a placeholder when it is missing.
    expect(build({
      historyStatus: "unavailable",
      historyFailure: hostError({ correlationId: "corr_abc" }),
    }).caveat?.correlationId).toBe("corr_abc");
    expect(build({
      historyStatus: "unavailable",
      historyFailure: hostError({ correlationId: null }),
    }).caveat?.correlationId).toBeNull();
    expect(build({
      historyStatus: "unavailable",
      historyError: "Something failed.",
    }).caveat?.correlationId).toBeNull();
  });

  it("prefers the structured failure's own message over a flattened one", () => {
    // Reason, correlation id and verdict have to come from one failure, or the
    // panel can pair a message with another failure's id.
    expect(build({
      historyStatus: "unavailable",
      historyError: "some older message",
      historyFailure: hostError({ safeMessage: "The transport dropped." }),
    }).caveat?.reason).toBe("The transport dropped.");
  });

  it("takes the retry verdict from the shared table, not a second opinion", () => {
    expect(build({
      historyStatus: "unavailable",
      historyFailure: hostError({ code: "transport_failed", retryable: true }),
    }).caveat?.recovery).toEqual({ kind: "retry" });
    expect(build({
      historyStatus: "unavailable",
      historyFailure: hostError({ code: "unauthorized", retryable: true }),
    }).caveat?.recovery).toEqual({ kind: "reauthenticate" });
    expect(build({
      historyStatus: "unavailable",
      historyFailure: hostError({ code: "integrity_failure", retryable: true }),
    }).caveat?.recovery).toEqual({ kind: "recover_local_store" });
  });

  it("withdraws the retry when the host called the failure final", () => {
    // Offering Refresh against a failure the host has ruled out is a control
    // that cannot succeed.
    const recovery = build({
      historyStatus: "unavailable",
      historyFailure: hostError({ code: "transport_failed", retryable: false }),
    }).caveat?.recovery;
    expect(recovery?.kind).toBe("none");
    expect(recovery?.kind === "none" && recovery.because).toContain("final");
  });

  it("leaves the verdict unstated when the shell handed it only a message", () => {
    // "Nobody has ruled on this" is not "nothing can be done", and must not be
    // shown as one.
    expect(build({
      historyStatus: "unavailable",
      historyError: "The host could not read the journal.",
    }).caveat?.recovery).toBeNull();
  });

  it("attaches no verdict and no correlation id to a caveat that is not a failure", () => {
    for (const journal of [
      build({ historyStatus: "unread" }),
      build({ historyStatus: "loading" }),
      build({ historyStatus: "loaded" }),
      build({ historyRecorded: false }),
    ]) {
      expect(journal.caveat?.recovery).toBeNull();
      expect(journal.caveat?.correlationId).toBeNull();
      expect(journal.caveat?.reason).toBeNull();
      expect(journal.caveat?.tone).toBe("neutral");
    }
  });

  it("gives every recovery a sentence", () => {
    const recoveries = [
      { kind: "retry" },
      { kind: "reauthenticate" },
      { kind: "regrant_workspace" },
      { kind: "review_context" },
      { kind: "recover_local_store" },
      { kind: "open_settings" },
      { because: "The host reported this as final.", kind: "none" },
    ] as const;
    const sentences = recoveries.map((recovery) => describeRecovery(recovery));
    expect(new Set(sentences).size).toBe(recoveries.length);
    for (const sentence of sentences) expect(sentence.endsWith(".")).toBe(true);
    // `none` already carries the host's own reason; restating it would replace
    // a specific explanation with a generic one.
    expect(sentences.at(-1)).toBe("The host reported this as final.");
  });
});
