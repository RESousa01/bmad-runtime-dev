import { Button } from "@sapphirus/ui";
import { useId } from "react";
import {
  type SkillVerdict,
  capabilityCompatibility,
  selectSkill,
  skillVerdict,
} from "../features/skills/skillAvailability";
import { capabilityFidelity } from "../lib/bmadFidelity";
import type {
  BmadAgentMenuProjection,
  BmadAvailability,
  BmadCoreSkillProjection,
  BmadHelpActionProjection,
  BmadInstalledSkillProjection,
  BmadLibraryProjection,
  BmadLibraryUiState,
  BmadMethodAgentProjection,
} from "../lib/bmadProjection";
import {
  CapabilityCoverageDisclosure,
  SkillBlockers,
  SkillCompatibility,
  SkillStartControl,
  SkillVerdictChip,
  SkillVerdictReason,
} from "./skills/SkillCompatibility";

export interface BmadPersonaPerspectiveView {
  readonly agentCode: string;
  readonly instructionMarkdown: string;
  readonly instructionHash: string;
}

export interface BmadLibraryPanelProps {
  readonly onRunBuilderCapability?: (capabilityId: string, label: string) => void;
  readonly onRunCapability?: (
    capabilityId: string,
    label: string,
  ) => void;
  readonly state: BmadLibraryUiState;
  readonly onReload?: () => void;
  readonly personaPerspectives?: ReadonlyMap<string, BmadPersonaPerspectiveView>;
  readonly onViewPersona?: (agentCode: string) => void;
}

const builderOperations = {
  agent: [
    ["builder:agent.analyze", "Analyze agent"],
    ["builder:agent.create_rebuild", "Create or rebuild agent"],
    ["builder:agent.edit", "Edit agent"],
  ],
  workflow: [
    ["builder:workflow.analyze", "Analyze workflow"],
    ["builder:workflow.build_edit", "Build or edit workflow"],
  ],
} as const;

function availabilityLabel(availability: BmadAvailability): string {
  switch (availability) {
    case "available":
      return "Available";
    case "capability_disabled":
      return "Capability disabled";
    case "dependency_unavailable":
      return "Dependency unavailable";
    case "orphan_skill":
      return "Catalog entry unavailable";
    case "network_unavailable":
      return "Network reference unavailable";
    case "source_prompt_unavailable":
      return "Source prompt unavailable";
  }
}

/**
 * The agent's own projected state.
 *
 * Deliberately *not* a skill verdict. An agent is a persona a menu hangs off,
 * not something this panel starts, and running it through `skillVerdict` would
 * report every agent as "not known" — each one is normalized into a package
 * that reaches no capability of its own, which is a fact about agents rather
 * than a doubt about them. The verdicts belong on the rows that offer a start.
 */
function AvailabilityChip({ availability }: { readonly availability: BmadAvailability }) {
  return (
    <span className="bmad-availability" data-availability={availability}>
      {availabilityLabel(availability)}
    </span>
  );
}

/**
 * Honest fidelity marker so a runnable Method capability is never presented as
 * the full upstream workflow. Every runnable capability runs as a single
 * reviewed one-shot request; this badge and its note say so.
 */
function FidelityBadge({ capabilityId }: { readonly capabilityId: string }) {
  const fidelity = capabilityFidelity(capabilityId);
  return (
    <span
      className="bmad-fidelity"
      data-fidelity={fidelity.level}
      title={fidelity.note}
    >
      {fidelity.label}
      {/* The note used to live only in `title`, which never reaches a keyboard
          or screen-reader user — so the honesty marker read as the bare word
          "Adapted" and explained nothing to the people least able to guess. */}
      <span className="sr-only"> — {fidelity.note}</span>
    </span>
  );
}

/**
 * The verdict for one agent menu.
 *
 * `upstreamSkill` is null because a menu carries none: it has a code, a label
 * and — when the build binds one — a capability. The matrix is keyed by upstream
 * skill directory, so it is consulted by capability further down the row
 * instead, and passing a name guessed off the capability id would be a claim
 * about upstream identity that nothing here holds.
 */
function menuVerdict(menu: BmadAgentMenuProjection): SkillVerdict {
  return skillVerdict({
    upstreamSkill: null,
    capabilityId: menu.capabilityId,
    projected: {
      availability: menu.availability,
      blockerCodes: menu.availabilityReason === null ? [] : [menu.availabilityReason],
    },
  });
}

/**
 * The verdict for one adopted Core-module skill.
 *
 * Membership of this list *is* the host's availability statement. The core-skill
 * projection is built from the capability table itself, so a row only reaches
 * here when this build binds a capability for it — which is why the projection
 * carries no availability field to read. Passing `null` instead would report
 * `bmm:bmad-shard-doc` and `bmm:bmad-index-docs` as not known and withdraw their
 * start controls: they are Forge-native and the matrix, whose denominator is
 * upstream directories, has no row for either.
 */
function coreSkillVerdict(skill: BmadCoreSkillProjection): SkillVerdict {
  return skillVerdict({
    upstreamSkill: skill.skill,
    capabilityId: skill.capabilityId,
    projected: { availability: "available", blockerCodes: [] },
  });
}

function InstalledSkillRow({ skill }: { readonly skill: BmadInstalledSkillProjection }) {
  // The skill directory name is the matrix's own key, and this row names no
  // capability -- an installed skill carries a module and a name only, so the
  // matrix's row is the one thing that knows what a start would run.
  const verdict = skillVerdict({
    upstreamSkill: skill.skillName,
    capabilityId: null,
    projected: { availability: skill.availability, blockerCodes: skill.blockerCodes },
  });
  return (
    <li className="bmad-library-row">
      <div className="bmad-library-row__heading">
        <strong>{skill.displayName}</strong>
        <SkillVerdictChip availability={verdict.availability} />
      </div>
      <p>{skill.description}</p>
      {skill.actions.length > 0 ? (
        <p>Declared actions: {skill.actions.join(", ")}</p>
      ) : (
        <p>No declared action alias.</p>
      )}
      {skill.hiddenFromHelp ? <p>Hidden from Help actions.</p> : null}
      <SkillCompatibility blockerCodes={skill.blockerCodes} verdict={verdict} />
    </li>
  );
}

function HelpActionRow({ helpAction }: { readonly helpAction: BmadHelpActionProjection }) {
  // Keyed by the skill, never by `skill:action`: the matrix records one row per
  // upstream directory, and both of `bmad-create-story`'s actions belong to it.
  const verdict = skillVerdict({
    upstreamSkill: helpAction.skillName,
    capabilityId: null,
    projected: {
      availability: helpAction.availability,
      blockerCodes: helpAction.blockerCodes,
    },
  });
  return (
    <li className="bmad-library-row">
      <div className="bmad-library-row__heading">
        <strong>{helpAction.displayName}</strong>
        <SkillVerdictChip availability={verdict.availability} />
      </div>
      <p>{helpAction.description}</p>
      {helpAction.menuCode === null ? <span>No menu code</span> : <span>Menu code {helpAction.menuCode}</span>}
      {helpAction.requiredGuidance ? <span>Required by BMAD guidance</span> : null}
      {helpAction.expectedArtifacts.length > 0 ? (
        <p>Expected artifacts: {helpAction.expectedArtifacts.join(", ")}</p>
      ) : (
        <p>No expected artifacts recorded.</p>
      )}
      <SkillCompatibility blockerCodes={helpAction.blockerCodes} verdict={verdict} />
    </li>
  );
}

function MethodAgentRow({
  agent,
  perspective,
  onViewPersona,
  onRunCapability,
  reviewedRunHelpId,
}: {
  readonly agent: BmadMethodAgentProjection;
  readonly perspective?: BmadPersonaPerspectiveView | undefined;
  readonly onViewPersona?: ((agentCode: string) => void) | undefined;
  readonly onRunCapability?:
    | ((capabilityId: string, label: string) => void)
    | undefined;
  readonly reviewedRunHelpId: string;
}) {
  // Split by the verdict rather than by `availability === "available" &&
  // capabilityId !== null` re-derived here. That inline test read a projection
  // defect -- a menu the catalog calls available while naming nothing to start
  // -- as a source-only prompt, and filed it under a heading that said so. The
  // verdict calls that case `unknown`, and the row now says which of the two it
  // is rather than the panel deciding for itself.
  const menuRows = agent.menus.map((menu) => ({ menu, verdict: menuVerdict(menu) }));
  const startable = menuRows.filter(
    ({ menu, verdict }) => selectSkill(verdict, menu.displayLabel).kind === "start",
  );
  const refused = menuRows.filter(
    ({ menu, verdict }) => selectSkill(verdict, menu.displayLabel).kind === "refused",
  );
  // Two lists under one agent used to carry the same `${name} menu` label, so
  // the reviewed actions and the refused ones were indistinguishable to
  // anything navigating by landmark or list name.
  const renderMenus = (rows: typeof menuRows, listLabel: string) => (
    <ul aria-label={listLabel} className="bmad-agent-menu">
      {rows.map(({ menu, verdict }) => (
        <li key={`${agent.moduleCode}\u0000${agent.agentCode}\u0000${menu.code}`}>
          <div>
            <strong>{menu.code}</strong>
            <span>
              {menu.targetKind === "prompt_reference"
                ? "Source-only prompt"
                : `Skill target · ${menu.displayLabel}`}
            </span>
          </div>
          <p>{menu.description}</p>
          <SkillVerdictChip availability={verdict.availability} />
          {verdict.availability.kind === "available" ? (
            <FidelityBadge capabilityId={verdict.availability.capabilityId} />
          ) : null}
          <SkillVerdictReason availability={verdict.availability} />
          {/* Was `<p>{menu.availabilityReason}</p>`: the bare blocker code,
              rendered in the position a reason belongs in. */}
          <SkillBlockers
            codes={menu.availabilityReason === null ? [] : [menu.availabilityReason]}
          />
          {menu.capabilityId === null ? null : (
            <CapabilityCoverageDisclosure
              compatibility={capabilityCompatibility(menu.capabilityId)}
            />
          )}
          <SkillStartControl
            describedBy={reviewedRunHelpId}
            label={menu.displayLabel}
            onStart={onRunCapability}
            verdict={verdict}
          />
        </li>
      ))}
    </ul>
  );
  return (
    <li aria-label={`${agent.name}, ${agent.title}`} className="bmad-agent-row">
      <div className="bmad-library-row__heading">
        <div>
          <span aria-hidden="true" className="bmad-agent-row__icon">{agent.icon}</span>
          <strong>{agent.name}</strong>
          <span>{agent.title}</span>
        </div>
        <AvailabilityChip availability={agent.availability} />
      </div>
      <p>{agent.description}</p>
      <p>{agent.team}</p>
      {onViewPersona && perspective === undefined ? (
        <button
          type="button"
          onClick={() => {
            onViewPersona(agent.agentCode);
          }}
        >
          View working stance
        </button>
      ) : null}
      {perspective === undefined ? null : (
        <section
          aria-label={`${agent.name} working stance`}
          className="bmad-agent-perspective"
        >
          <pre>{perspective.instructionMarkdown}</pre>
        </section>
      )}
      <SkillBlockers codes={agent.blockerCodes} />
      {startable.length > 0
        ? renderMenus(startable, `${agent.name} reviewed actions`)
        : null}
      {refused.length > 0 ? (
        // "Source-only" was a claim about *why*, and it was wrong for every
        // skill-target menu the build could not bind. The group now names only
        // what is true of all of them -- no start is offered here -- and each
        // row inside carries its own verdict and reason.
        <details className="bmad-source-actions">
          <summary>{refused.length} {refused.length === 1 ? "action" : "actions"} that cannot be started here</summary>
          {renderMenus(refused, `${agent.name} actions that cannot be started`)}
        </details>
      ) : startable.length === 0 ? (
        <p>No reviewed actions.</p>
      ) : (
        null
      )}
    </li>
  );
}

function InternalIdentifiers({ projection }: { readonly projection: BmadLibraryProjection }) {
  return (
    <details className="bmad-internal-identifiers">
      <summary>Internal identifiers</summary>
      <div>
        <h3>Source identity</h3>
        <dl>
          <div><dt>Package</dt><dd><code>{projection.source.packageName}</code></dd></div>
          <div><dt>Version</dt><dd><code>{projection.source.packageVersion}</code></dd></div>
          <div><dt>Scope</dt><dd><code>{projection.scope}</code></dd></div>
        </dl>

        <h3>Installed skill identities</h3>
        {projection.installedSkills.length > 0 ? (
          <ul>
            {projection.installedSkills.map((skill) => (
              <li key={`${skill.moduleCode}\u0000${skill.skillName}`}>
                <code>{skill.moduleCode} / {skill.skillName}</code>
                <dl>
                  <div><dt>Entrypoint kind</dt><dd><code>{skill.entrypointKind}</code></dd></div>
                  <div><dt>Distribution profile</dt><dd><code>{skill.distributionProfile}</code></dd></div>
                  <div><dt>Installation profile</dt><dd><code>{skill.installProfile}</code></dd></div>
                  <div><dt>Validation profile</dt><dd><code>{skill.validationProfile}</code></dd></div>
                </dl>
              </li>
            ))}
          </ul>
        ) : <p>No installed skill identifiers.</p>}

        <h3>Action identities</h3>
        {projection.helpActions.length > 0 ? (
          <ul>
            {projection.helpActions.map((helpAction) => (
              <li key={`${helpAction.moduleCode}\u0000${helpAction.skillName}\u0000${helpAction.action ?? ""}`}>
                <code>
                  {helpAction.moduleCode} / {helpAction.skillName} / {helpAction.action ?? "no action"} / {helpAction.menuCode ?? "no menu code"}
                </code>
              </li>
            ))}
          </ul>
        ) : <p>No action identifiers.</p>}

        <h3>Agent and menu identities</h3>
        {projection.methodAgents.length > 0 ? (
          <ul>
            {projection.methodAgents.map((agent) => (
              <li key={`${agent.moduleCode}\u0000${agent.agentCode}`}>
                <code>{agent.moduleCode} / {agent.agentCode}</code>
                {agent.menus.length > 0 ? (
                  <ul>
                    {agent.menus.map((menu) => (
                      <li key={`${agent.moduleCode}\u0000${agent.agentCode}\u0000${menu.code}`}>
                        <code>{agent.moduleCode} / {agent.agentCode} / {menu.code}</code>
                      </li>
                    ))}
                  </ul>
                ) : null}
              </li>
            ))}
          </ul>
        ) : <p>No agent or menu identifiers.</p>}
      </div>
    </details>
  );
}

function ReadyLibrary({
  projection,
  personaPerspectives,
  onViewPersona,
  onRunCapability,
  onRunBuilderCapability,
}: {
  readonly projection: BmadLibraryProjection;
  readonly personaPerspectives?: ReadonlyMap<string, BmadPersonaPerspectiveView> | undefined;
  readonly onViewPersona?: ((agentCode: string) => void) | undefined;
  readonly onRunCapability?:
    | ((capabilityId: string, label: string) => void)
    | undefined;
  readonly onRunBuilderCapability?:
    | ((capabilityId: string, label: string) => void)
    | undefined;
}) {
  const skillsHeadingId = useId();
  const actionsHeadingId = useId();
  const agentsHeadingId = useId();
  const coreSkillsHeadingId = useId();
  const builderHeadingId = useId();
  const reviewedRunHelpId = useId();

  // The explanation is rendered only where a startable control actually exists.
  // A catalog with nothing startable should not carry instructions for a button
  // that is not on the page. Asked of `selectSkill`, the same gate the controls
  // themselves go through, so the help text cannot appear over a catalog whose
  // every start the verdict refused.
  const hasStartableCapability = (onRunCapability !== undefined
    && (projection.coreSkills.some(
      (skill) => selectSkill(coreSkillVerdict(skill), skill.displayName).kind === "start",
    )
      || projection.methodAgents.some((agent) =>
        agent.menus.some(
          (menu) => selectSkill(menuVerdict(menu), menu.displayLabel).kind === "start",
        ))))
    || (onRunBuilderCapability !== undefined && projection.builderPackages.length > 0);

  return (
    <>
      <p className="bmad-library-panel__source">
        {projection.source.packageName} {projection.source.packageVersion} · Read only
      </p>

      {hasStartableCapability ? (
        // What "Start reviewed run" means, next to the controls that do it.
        // The label names the mechanism and not the promise, and nothing else
        // on this screen said what is reviewed, by whom, or when.
        <p className="bmad-library-panel__reviewed-run-help" id={reviewedRunHelpId}>
          <strong>Start reviewed run</strong> does not send your workspace. You
          choose the exact files, read them back, and approve them — only that
          approved context is sent, once. Any file change that comes back is a
          proposal you approve in Changes before anything is written.
        </p>
      ) : null}

      <section aria-labelledby={agentsHeadingId} className="bmad-library-section">
        <h3 id={agentsHeadingId}>Reviewed agents</h3>
        {projection.methodAgents.length > 0 ? (
          <ul>
            {projection.methodAgents.map((agent) => (
              <MethodAgentRow
                agent={agent}
                onRunCapability={onRunCapability}
                key={`${agent.moduleCode}\u0000${agent.agentCode}`}
                onViewPersona={onViewPersona}
                perspective={personaPerspectives?.get(agent.agentCode)}
                reviewedRunHelpId={reviewedRunHelpId}
              />
            ))}
          </ul>
        ) : <p>No agents available.</p>}
      </section>

      {projection.coreSkills.length > 0 ? (
        <section aria-labelledby={coreSkillsHeadingId} className="bmad-library-section">
          <h3 id={coreSkillsHeadingId}>Core skills</h3>
          <p>
            Adopted upstream skills that belong to the Core module rather than
            to an agent. Upstream invokes several of them from inside other
            skills; each also runs on its own.
          </p>
          <ul>
            {projection.coreSkills.map((skill) => {
              const verdict = coreSkillVerdict(skill);
              return (
                <li className="bmad-library-row" key={skill.capabilityId}>
                  <div>
                    <strong>{skill.displayName}</strong>
                    <span>Menu code {skill.menuCode}</span>
                    <SkillVerdictChip availability={verdict.availability} />
                    {verdict.availability.kind === "available" ? (
                      <FidelityBadge capabilityId={verdict.availability.capabilityId} />
                    ) : null}
                  </div>
                  <SkillCompatibility verdict={verdict} />
                  {/* The start used to be unconditional here, so a core skill
                      the matrix contradicts would have been offered and only
                      failed once the user had spent a reviewed context on it. */}
                  <SkillStartControl
                    describedBy={reviewedRunHelpId}
                    label={skill.displayName}
                    onStart={onRunCapability}
                    verdict={verdict}
                  />
                </li>
              );
            })}
          </ul>
        </section>
      ) : null}

      {projection.builderPackages.length > 0 ? (
        <section aria-labelledby={builderHeadingId} className="bmad-library-section">
          <h3 id={builderHeadingId}>Builder (installed, inactive)</h3>
          <ul>
            {projection.builderPackages.map((builder) => (
              <li
                className="bmad-library-row"
                key={`${builder.packageName}\u0000${builder.packageKind}`}
              >
                <div className="bmad-library-row__heading">
                  <strong>{builder.displayName}</strong>
                  <span className="bmad-availability" data-availability="inactive">Inactive output</span>
                </div>
                <p>
                  {builder.packageName} {builder.packageVersion} ·{" "}
                  {builder.packageKind === "agent" ? "Agent package" : "Workflow package"} ·{" "}
                  {builder.resourceCount === 1
                    ? "1 resource"
                    : `${builder.resourceCount} resources`}{" "}
                  · <code>{builder.descriptorDigest}</code>
                </p>
                <p className="bmad-library-row__note">
                  Builder results remain inert drafts. They cannot install, register, or execute.
                </p>
                {onRunBuilderCapability === undefined ? null : (
                  <div className="bmad-builder-actions">
                    {builderOperations[builder.packageKind].map(([capabilityId, label]) => (
                      <Button
                        // These read as immediate verbs but open the same
                        // reviewed run as everything else on this screen, so
                        // they carry the same explanation.
                        aria-describedby={reviewedRunHelpId}
                        key={capabilityId}
                        onPress={() => onRunBuilderCapability(capabilityId, label)}
                        size="small"
                        variant="secondary"
                      >
                        {label}
                      </Button>
                    ))}
                  </div>
                )}
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      <details className="bmad-source-details">
        <summary>
          Source catalog details
          <span>{projection.installedSkills.length + projection.helpActions.length} entries</span>
        </summary>
        <section aria-labelledby={skillsHeadingId} className="bmad-library-section">
          <h3 id={skillsHeadingId}>Installed skills</h3>
          {projection.installedSkills.length > 0 ? (
            <ul>
              {projection.installedSkills.map((skill) => (
                <InstalledSkillRow
                  key={`${skill.moduleCode}\u0000${skill.skillName}`}
                  skill={skill}
                />
              ))}
            </ul>
          ) : <p>No installed skills available.</p>}
        </section>

        <section aria-labelledby={actionsHeadingId} className="bmad-library-section">
          <h3 id={actionsHeadingId}>Source actions</h3>
          {projection.helpActions.length > 0 ? (
            <ul>
              {projection.helpActions.map((helpAction) => (
                <HelpActionRow
                  helpAction={helpAction}
                  key={`${helpAction.moduleCode}\u0000${helpAction.skillName}\u0000${helpAction.action ?? ""}`}
                />
              ))}
            </ul>
          ) : <p>No source actions recorded.</p>}
        </section>

        <InternalIdentifiers projection={projection} />
      </details>
    </>
  );
}

export function BmadLibraryPanel({
  onReload,
  onRunBuilderCapability,
  onRunCapability,
  onViewPersona,
  personaPerspectives,
  state,
}: BmadLibraryPanelProps) {
  const headingId = useId();
  let body;

  switch (state.kind) {
    case "idle":
      // Reached on first paint and again after every host rebind or workspace
      // revocation. With no control it was a terminal state that read like a
      // failure: the catalog could never be asked for from here.
      body = (
        <div>
          <p>This workspace's skills and agents have not been read yet.</p>
          {onReload ? (
            <Button onPress={onReload} size="small" variant="secondary">
              Read skills and agents
            </Button>
          ) : (
            <p>Reading them needs the Forge desktop host, which is not bound right now.</p>
          )}
        </div>
      );
      break;
    case "loading":
      body = <p aria-live="polite" role="status">Loading skills and agents catalog…</p>;
      break;
    case "unavailable": {
      const message = state.message.trim() || "The skills and agents catalog is unavailable.";
      body = (
        <div>
          <p role="alert">{message}</p>
          {state.retryable ? null : (
            <p>
              The host did not report this as a temporary failure, so reading
              again will most likely give the same answer until whatever caused
              it is fixed.
            </p>
          )}
          {/* The reload used to be gated on `retryable`, which left every
              non-retryable failure with no control at all — a dead panel whose
              only escape was restarting the app. Reading the catalog is a
              local, side-effect-free host read, so offering it again costs
              nothing and is the only way back once the cause is fixed. */}
          {onReload ? (
            <Button onPress={onReload} size="small" variant="secondary">
              Reload skills and agents
            </Button>
          ) : null}
        </div>
      );
      break;
    }
    case "ready":
      body = (
        <ReadyLibrary
          onRunBuilderCapability={onRunBuilderCapability}
          onRunCapability={onRunCapability}
          onViewPersona={onViewPersona}
          personaPerspectives={personaPerspectives}
          projection={state.projection}
        />
      );
      break;
  }

  return (
    <section
      aria-busy={state.kind === "loading" || undefined}
      aria-labelledby={headingId}
      className="bmad-library-panel"
    >
      <h2 id={headingId}>Skills &amp; agents</h2>
      {body}
    </section>
  );
}
