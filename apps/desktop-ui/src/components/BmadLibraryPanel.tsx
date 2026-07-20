import { Button } from "@sapphirus/ui";
import { useId } from "react";
import type {
  BmadAvailability,
  BmadHelpActionProjection,
  BmadInstalledSkillProjection,
  BmadLibraryProjection,
  BmadLibraryUiState,
  BmadMethodAgentProjection,
} from "../lib/bmadProjection";

export interface BmadPersonaPerspectiveView {
  readonly agentCode: string;
  readonly instructionMarkdown: string;
  readonly instructionHash: string;
}

export interface BmadLibraryPanelProps {
  readonly onRunCapability?: (
    agentCode: string,
    menuCode: string,
    label: string,
  ) => void;
  readonly state: BmadLibraryUiState;
  readonly onReload?: () => void;
  readonly personaPerspectives?: ReadonlyMap<string, BmadPersonaPerspectiveView>;
  readonly onViewPersona?: (agentCode: string) => void;
}

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

function Blockers({ codes }: { readonly codes: readonly string[] }) {
  if (codes.length === 0) {
    return null;
  }
  return (
    <div className="bmad-blockers">
      <span>Blockers</span>
      <ul aria-label="Blockers">
        {codes.map((code) => <li key={code}><code>{code}</code></li>)}
      </ul>
    </div>
  );
}

function InstalledSkillRow({ skill }: { readonly skill: BmadInstalledSkillProjection }) {
  return (
    <li className="bmad-library-row">
      <div className="bmad-library-row__heading">
        <strong>{skill.displayName}</strong>
        <span className="bmad-availability">{availabilityLabel(skill.availability)}</span>
      </div>
      <p>{skill.description}</p>
      {skill.actions.length > 0 ? (
        <p>Declared actions: {skill.actions.join(", ")}</p>
      ) : (
        <p>No declared action alias.</p>
      )}
      {skill.hiddenFromHelp ? <p>Hidden from Help actions.</p> : null}
      <Blockers codes={skill.blockerCodes} />
    </li>
  );
}

function HelpActionRow({ helpAction }: { readonly helpAction: BmadHelpActionProjection }) {
  return (
    <li className="bmad-library-row">
      <div className="bmad-library-row__heading">
        <strong>{helpAction.displayName}</strong>
        <span className="bmad-availability">{availabilityLabel(helpAction.availability)}</span>
      </div>
      <p>{helpAction.description}</p>
      {helpAction.menuCode === null ? <span>No menu code</span> : <span>Menu code {helpAction.menuCode}</span>}
      {helpAction.requiredGuidance ? <span>Required by BMAD guidance</span> : null}
      {helpAction.expectedArtifacts.length > 0 ? (
        <p>Expected artifacts: {helpAction.expectedArtifacts.join(", ")}</p>
      ) : (
        <p>No expected artifacts recorded.</p>
      )}
      <Blockers codes={helpAction.blockerCodes} />
    </li>
  );
}

function MethodAgentRow({
  agent,
  perspective,
  onViewPersona,
  onRunCapability,
}: {
  readonly agent: BmadMethodAgentProjection;
  readonly perspective?: BmadPersonaPerspectiveView | undefined;
  readonly onViewPersona?: ((agentCode: string) => void) | undefined;
  readonly onRunCapability?:
    | ((agentCode: string, menuCode: string, label: string) => void)
    | undefined;
}) {
  return (
    <li aria-label={`${agent.name}, ${agent.title}`} className="bmad-agent-row">
      <div className="bmad-library-row__heading">
        <div>
          <span aria-hidden="true" className="bmad-agent-row__icon">{agent.icon}</span>
          <strong>{agent.name}</strong>
          <span>{agent.title}</span>
        </div>
        <span className="bmad-availability">{availabilityLabel(agent.availability)}</span>
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
      <Blockers codes={agent.blockerCodes} />
      {agent.menus.length > 0 ? (
        <ul aria-label={`${agent.name} menu`} className="bmad-agent-menu">
          {agent.menus.map((menu) => (
            <li key={`${agent.moduleCode}\u0000${agent.agentCode}\u0000${menu.code}`}>
              <div>
                <strong>{menu.code}</strong>
                <span>
                  {menu.targetKind === "prompt_reference"
                    ? "Source prompt reference — unavailable"
                    : `Skill target · ${menu.displayLabel}`}
                </span>
              </div>
              <p>{menu.description}</p>
              <span className="bmad-availability">{availabilityLabel(menu.availability)}</span>
              {menu.availabilityReason === null ? null : <p>{menu.availabilityReason}</p>}
              {onRunCapability === undefined ? null : (
                <button
                  type="button"
                  onClick={() => {
                    onRunCapability(agent.agentCode, menu.code, menu.displayLabel);
                  }}
                >
                  Start reviewed run
                </button>
              )}
            </li>
          ))}
        </ul>
      ) : (
        <p>No descriptive menu rows.</p>
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
}: {
  readonly projection: BmadLibraryProjection;
  readonly personaPerspectives?: ReadonlyMap<string, BmadPersonaPerspectiveView> | undefined;
  readonly onViewPersona?: ((agentCode: string) => void) | undefined;
  readonly onRunCapability?:
    | ((agentCode: string, menuCode: string, label: string) => void)
    | undefined;
}) {
  const skillsHeadingId = useId();
  const actionsHeadingId = useId();
  const agentsHeadingId = useId();
  const builderHeadingId = useId();

  return (
    <>
      <p className="bmad-library-panel__source">
        {projection.source.packageName} {projection.source.packageVersion} · Read only
      </p>

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
        <h3 id={actionsHeadingId}>Available actions</h3>
        {projection.helpActions.length > 0 ? (
          <ul>
            {projection.helpActions.map((helpAction) => (
              <HelpActionRow
                helpAction={helpAction}
                key={`${helpAction.moduleCode}\u0000${helpAction.skillName}\u0000${helpAction.action ?? ""}`}
              />
            ))}
          </ul>
        ) : <p>No available actions.</p>}
      </section>

      <section aria-labelledby={agentsHeadingId} className="bmad-library-section">
        <h3 id={agentsHeadingId}>Agents</h3>
        {projection.methodAgents.length > 0 ? (
          <ul>
            {projection.methodAgents.map((agent) => (
              <MethodAgentRow
                agent={agent}
                onRunCapability={onRunCapability}
                key={`${agent.moduleCode}\u0000${agent.agentCode}`}
                onViewPersona={onViewPersona}
                perspective={personaPerspectives?.get(agent.agentCode)}
              />
            ))}
          </ul>
        ) : <p>No agents available.</p>}
      </section>

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
                  <span className="bmad-availability">Inactive</span>
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
                  Activation requires a local decision and is not yet available.
                </p>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      <InternalIdentifiers projection={projection} />
    </>
  );
}

export function BmadLibraryPanel({
  onReload,
  onRunCapability,
  onViewPersona,
  personaPerspectives,
  state,
}: BmadLibraryPanelProps) {
  const headingId = useId();
  let body;

  switch (state.kind) {
    case "idle":
      body = <p>No skills and agents catalog requested.</p>;
      break;
    case "loading":
      body = <p aria-live="polite" role="status">Loading skills and agents catalog…</p>;
      break;
    case "unavailable": {
      const message = state.message.trim() || "The skills and agents catalog is unavailable.";
      body = (
        <div>
          <p role="alert">{message}</p>
          {state.retryable && onReload ? (
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
      <h2 id={headingId}>BMAD library</h2>
      {body}
    </section>
  );
}
