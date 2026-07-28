import { Button } from "@sapphirus/ui";
import { Trash2 } from "lucide-react";
import { useState } from "react";
import type {
  RetentionCategory,
  RetentionManifestProjection,
} from "../../lib/hostClient";
import { SettingsRow } from "./SettingsRow";

export type OffboardingStatus =
  | "loading"
  | "unavailable"
  | "ready"
  | "erasing"
  | "erased";

// Must match the host-validated ADR-0004 phrase byte for byte.
export const OFFBOARDING_ERASE_CONFIRMATION = "erase-local-authority-data";

/// The same categories the host reports, arranged so the panel answers "what is
/// this" instead of only "how many rows".
///
/// Every one of the twenty-three rows used to carry the identical sentence
/// "Kept under the app's local authority store", which is a fact about the
/// list, not about any row in it. It is stated once above the list now, and
/// the explanation that varies — what kind of record this is — is stated once
/// per family.
///
/// This is presentation only. The `categoryLabels` prop stays the pinned
/// one-to-one map of every category the host reports; a category that appears
/// in no family below is still shown, under "Other records", so a new host
/// category can never go missing from this panel.
const retentionGroups: ReadonlyArray<{
  readonly categories: readonly string[];
  readonly detail: string;
  readonly id: string;
  readonly title: string;
}> = [
  {
    id: "conversations",
    title: "Conversations",
    detail:
      "The tasks you have opened and every message in them, including what the agent replied.",
    categories: ["chat_conversations", "chat_messages"],
  },
  {
    id: "authority",
    title: "Workspace grants and evidence",
    detail:
      "Which folders you granted access to, the approvals you spent, and the record of what was done under them.",
    categories: [
      "workspace_and_authority_records",
      "approval_consumptions",
      "evidence_events",
      "stored_payloads",
    ],
  },
  {
    id: "changes",
    title: "Governed changes",
    detail:
      "Edits proposed against your workspace, the checkpoint that makes applying one reversible, and the outcome.",
    categories: ["execution_journals", "execution_checkpoints", "execution_results"],
  },
  {
    id: "method",
    title: "Skills, agents, and runs",
    detail:
      "Work driven by the bundled skills and agents: the runs themselves, what they produced, and the approvals you granted them.",
    categories: [
      "method_sessions",
      "method_checkpoints",
      "method_artifacts",
      "method_decision_consumptions",
      "capability_runs",
      "capability_results",
      "help_runs",
      "memlog_entries",
      "architecture_sessions",
      "architecture_session_steps",
    ],
  },
  {
    id: "builder",
    title: "Builder authoring",
    detail:
      "Skill and agent drafts you authored here, their revision history, and the analyses run over them.",
    categories: [
      "builder_drafts",
      "builder_draft_revisions",
      "builder_analyses",
      "builder_analysis_decisions",
    ],
  },
];

export function groupRetentionCategories(
  categories: readonly RetentionCategory[],
): ReadonlyArray<{
  readonly detail: string;
  readonly entries: readonly RetentionCategory[];
  readonly id: string;
  readonly title: string;
}> {
  const placed = new Set(retentionGroups.flatMap((group) => group.categories));
  const ungrouped = categories.filter((entry) => !placed.has(entry.category));
  return [
    ...retentionGroups
      .map((group) => ({
        detail: group.detail,
        entries: categories.filter((entry) => group.categories.includes(entry.category)),
        id: group.id,
        title: group.title,
      }))
      .filter((group) => group.entries.length > 0),
    ...(ungrouped.length > 0
      ? [
          {
            detail: "Kept in the same local store as the categories above.",
            entries: ungrouped,
            id: "other",
            title: "Other records",
          },
        ]
      : []),
  ];
}

export function formatRetainedBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/// The label for a category the pinned map does not name.
///
/// Only reachable if the host starts reporting a category the map has not
/// caught up with, which the boundary gate exists to prevent. It is still here
/// because a panel whose stated job is "what does this app keep" must never
/// silently omit a row: an ugly humanised table name beats a missing one.
function categoryLabel(
  labels: Readonly<Record<string, string>>,
  category: string,
): string {
  return (
    labels[category]
    ?? category.replaceAll("_", " ").replace(/^./u, (c) => c.toUpperCase())
  );
}

/**
 * What the app keeps on this device, and the one control that destroys it.
 *
 * The seam is the irreversible act. Everything on this pane exists to make
 * erasing informed and expensive — the grouped counts answer "what would I
 * lose", the typed phrase is the price — and nothing else in Settings can
 * destroy data. Reviewing that guard should not require reading the provider
 * form or the theme picker, and changing the theme picker must not be able to
 * reach it.
 *
 * The confirmation phrase is state here rather than a prop because it is the
 * arming mechanism itself: a caller that could set it could arm the button, and
 * the point of the phrase is that only typing it does.
 *
 * `categoryLabels` arrives as data, which looks like indirection for its own
 * sake and is not. `tools/check-boundaries.mjs` pins that map against
 * `RETENTION_CATEGORIES` in `crates/desktop-store/src/lib.rs` by reading
 * `SettingsDialog.tsx` at a literal path, so moving the map into this file
 * would not fail that gate — it would make it match nothing and stop checking.
 * The map stays where the gate looks. Taking it as a prop also lets a test hand
 * in two entries instead of all twenty-three.
 */
export interface LocalDataSectionProps {
  readonly categoryLabels: Readonly<Record<string, string>>;
  readonly manifest: RetentionManifestProjection | null;
  readonly notice: string | null;
  readonly onErase: (confirm: string) => void;
  readonly status: OffboardingStatus;
}

export function LocalDataSection({
  categoryLabels,
  manifest,
  notice,
  onErase,
  status,
}: LocalDataSectionProps) {
  const [eraseConfirmation, setEraseConfirmation] = useState("");
  const eraseArmed = eraseConfirmation === OFFBOARDING_ERASE_CONFIRMATION;

  return (
    <section aria-labelledby="settings-offboarding-title">
      <header className="settings-pane__header">
        <h3 id="settings-offboarding-title">Local data</h3>
        <p>
          What this app keeps on this device, and how to erase it.
          Uninstalling keeps this data unless you explicitly ask the
          uninstaller to erase it; your workspace files are yours and
          are never touched.
        </p>
      </header>
      {status === "erased" ? (
        <p className="settings-pane__copy" role="status">
          Local data was erased. The session is now read-only; restart
          the app to start with a fresh identity.
        </p>
      ) : status === "unavailable" ? (
        <p className="settings-pane__copy" role="status">
          Local data details are unavailable in this mode.
        </p>
      ) : manifest === null ? (
        <p className="settings-pane__copy" role="status">
          Loading local data details…
        </p>
      ) : (
        <>
          <p className="retention-lede">
            Each number below counts records, not files, held in the
            app&rsquo;s local authority store on this device.
          </p>
          <div className="retention-groups">
            {groupRetentionCategories(manifest.categories).map((group) => (
              <div className="retention-group" key={group.id}>
                <h4>{group.title}</h4>
                <p>{group.detail}</p>
                <dl className="retention-group__counts">
                  {group.entries.map((entry) => (
                    <div key={entry.category}>
                      <dt>{categoryLabel(categoryLabels, entry.category)}</dt>
                      <dd>{entry.count}</dd>
                    </div>
                  ))}
                </dl>
              </div>
            ))}
          </div>
          <div className="settings-list">
            <SettingsRow
              detail="Encrypted local store, database, and key material."
              label="Total retained"
              value={formatRetainedBytes(manifest.retainedBytes)}
            />
          </div>
          <fieldset
            aria-label="Erase local data"
            className="settings-choice-group settings-choice-group--danger"
          >
            <legend>Erase local data</legend>
            <p className="settings-pane__copy">
              Erasing signs you out, revokes every workspace grant, and
              destroys the local store key so remaining bytes are
              undecryptable. This cannot be undone. Type{" "}
              <code>{OFFBOARDING_ERASE_CONFIRMATION}</code> to confirm.
            </p>
            <input
              aria-label="Erase confirmation phrase"
              autoComplete="off"
              className="settings-erase-confirmation"
              onChange={(event) => setEraseConfirmation(event.target.value)}
              placeholder={OFFBOARDING_ERASE_CONFIRMATION}
              spellCheck={false}
              type="text"
              value={eraseConfirmation}
            />
            <Button
              className="settings-pane__action"
              isDisabled={!eraseArmed || status === "erasing"}
              onPress={() => onErase(eraseConfirmation)}
              variant="secondary"
            >
              <Trash2 aria-hidden="true" size={15} />{" "}
              {status === "erasing"
                ? "Erasing local data…"
                : "Erase all local data"}
            </Button>
          </fieldset>
        </>
      )}
      {notice ? (
        <p className="settings-pane__copy" role="status">
          {notice}
        </p>
      ) : null}
    </section>
  );
}
