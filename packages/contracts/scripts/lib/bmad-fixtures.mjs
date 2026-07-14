import { canonicalHash } from "./canonical-json.mjs";
import { sealDocument } from "./semantics.mjs";

const NOW = "2026-07-14T10:00:00.000Z";
const PACKAGE_VERSION_ID = id("pkgver");
const MODULE_HASH = exactHash(
  "5a2a4ff761b3a4f92730442386486f32318152fc0dfdd225dc6765a3bc2ec100",
);
const CORE_MODULE_HASH = exactHash(
  "46f8972746f0d4e49358fdf94b0c1ba856fd7a8eb66abc75d5aaff0624540479",
);
const CORE_HELP_HASH = exactHash(
  "e801caeb1bf6484277867067c60be3c2aeec39beaa75254e64ddf8ce8f3b617d",
);
const BMM_HELP_HASH = exactHash(
  "ad4373d7e58a31aaef601ae39cf76b26bae7fd420b108e44660427384652d4bf",
);
const METHOD_ARCHIVE_HASH = exactHash(
  "a7c049038099b99081fbd03d22c6a5180edd88dee656bb37c4276b1cc31b4a32",
);
const BMAD_HELP_SOURCE_HASH = exactHash(
  "718077d741e20d9c94f3c2b7827047f2d18a90b85c3cc2eecd449e28b7b0d642",
);
const ARCHITECTURE_SOURCE_HASH = exactHash(
  "a56c7a0abc45e1dba719ae5e66b7169a1098b403e7fd69b30c19f16c12cddc6a",
);
const ARCHITECTURE_CUSTOMIZATION_HASH = exactHash(
  "137b418e1bb940411a6e77460a7c74af66a6ad732edcc7c7363746995b89e65d",
);
const MANAGED_HELP_HASH = exactHash(
  "d3d3c91d516d32546c446503d88957f6e499c504370b6749b5936f786643df66",
);
const MANAGED_ARCHITECT_PERSONA_HASH = exactHash(
  "6d3512c6f9014a2344418ce0b53b1c9ed8521e6bf8b337f2a802ade6307146e4",
);
const MANAGED_ARCHITECTURE_HASH = exactHash(
  "57b356fb427008f181c1a6027e8efbf7d177d4b45dad4d15612c84213999d6f0",
);

const AGENTS = Object.freeze([
  Object.freeze({
    agentCode: "bmad-agent-analyst",
    name: "Mary",
    title: "Business Analyst",
    icon: "📊",
    team: "software-development",
    description: "Channels Porter's strategic rigor and Minto's Pyramid Principle, grounds every finding in verifiable evidence, represents every stakeholder voice. Speaks like a treasure hunter narrating the find: thrilled by every clue, precise once the pattern emerges.",
    relative: "1-analysis/bmad-agent-analyst",
    personaHex: "fca38a404e79508ebc39235d0162bcb475b02407a913e3e5bd8c5526b2b261a6",
    customizationHex: "7191f4a60ada7dbabe083699b2461f35778af7688924dc6ce05911cf1ffc9054",
    menu: [
      ["BP", "Brainstorm Project", "Expert guided brainstorming facilitation", "bmad-brainstorming", null],
      ["MR", "Market Research", "Market analysis, competitive landscape, customer needs and trends", "bmad-market-research", null],
      ["DR", "Domain Research", "Industry domain deep dive, subject matter expertise and terminology", "bmad-domain-research", null],
      ["TR", "Technical Research", "Technical feasibility, architecture options and implementation approaches", "bmad-technical-research", null],
      ["CB", "Create Brief", "Create or update product briefs through guided or autonomous discovery", "bmad-product-brief", null],
      ["WB", "PRFAQ Challenge", "Working Backwards PRFAQ challenge — forge and stress-test product concepts", "bmad-prfaq", null],
      ["DP", "Document Project", "Analyze an existing project to produce documentation for human and LLM consumption", "bmad-document-project", null],
    ],
  }),
  Object.freeze({
    agentCode: "bmad-agent-architect",
    name: "Winston",
    title: "System Architect",
    icon: "🏗️",
    team: "software-development",
    description: "Favors boring technology for stability, developer productivity as architecture, ties every decision to business value. Speaks like a seasoned engineer at the whiteboard: measured, always laying out trade-offs rather than verdicts.",
    relative: "3-solutioning/bmad-agent-architect",
    personaHex: "82bbf22ff3a4571741c9339ae6b7c35676a6e26a3237ceca487abe85369e91a0",
    customizationHex: "d9763009d7c20246119c24bcea5eacebd21ad60c22ab191b74c9a5fb6e5f57ad",
    menu: [
      ["CA", "Architecture", "Produce the architecture spine: the invariants that keep independently-built units consistent", "bmad-architecture", "create"],
      ["IR", "Check Implementation Readiness", "Ensure the PRD, UX, Architecture and Epics and Stories List are all aligned", "bmad-check-implementation-readiness", null],
    ],
  }),
  Object.freeze({
    agentCode: "bmad-agent-dev",
    name: "Amelia",
    title: "Senior Software Engineer",
    icon: "💻",
    team: "software-development",
    description: "Test-first discipline (red, green, refactor), 100% pass before review, no fluff all precision. Speaks like a terminal prompt: exact file paths, AC IDs, and commit-message brevity — every statement citable.",
    relative: "4-implementation/bmad-agent-dev",
    personaHex: "54ab6d9d60d7ed7e84d6510cf325734b66feb1b419131436672559584b4bf508",
    customizationHex: "01a45ac2420f920cfc934de7b1a884b04017c85641085c93d9aada45d2dccf17",
    menu: [
      ["DS", "Dev Story", "Write the next or specified story's tests and code", "bmad-dev-story", null],
      ["QD", "Quick Dev", "Unified quick flow — clarify intent, plan, implement, review, present", "bmad-quick-dev", null],
      ["QA", "QA Automation Test", "Generate API and E2E tests for existing features", "bmad-qa-generate-e2e-tests", null],
      ["CR", "Code Review", "Initiate a comprehensive code review across multiple quality facets", "bmad-code-review", null],
      ["SP", "Sprint Planning", "Generate or update the sprint plan that sequences tasks for implementation", "bmad-sprint-planning", null],
      ["CS", "Create Story", "Prepare a story with all required context for implementation", "bmad-create-story", null],
      ["ER", "Retrospective", "Party mode review of all work completed across an epic", "bmad-retrospective", null],
    ],
  }),
  Object.freeze({
    agentCode: "bmad-agent-pm",
    name: "John",
    title: "Product Manager",
    icon: "📋",
    team: "software-development",
    description: "Drives Jobs-to-be-Done over template filling, user value first, technical feasibility is a constraint not the driver. Speaks like a detective interrogating a cold case: short questions, sharper follow-ups, every 'why?' tightening the net.",
    relative: "2-plan-workflows/bmad-agent-pm",
    personaHex: "e2ebea0bf6ee158fcb9e62731a185983f2ac59f664940f4f4320cf10e1bbdf92",
    customizationHex: "afc2250de25241ac3577c9827e1ee62d8c7a1913de8eb23aadfffb0aadbadc37",
    menu: [
      ["PRD", "Create Edit and Review PRD", "Create, update, or validate a PRD — state your intent or the skill will ask", "bmad-prd", null],
      ["CE", "Create Epics and Stories", "Create the Epics and Stories Listing that will drive development", "bmad-create-epics-and-stories", null],
      ["IR", "Check Implementation Readiness", "Ensure the PRD, UX, Architecture and Epics and Stories List are all aligned", "bmad-check-implementation-readiness", null],
      ["CC", "Correct Course", "Determine how to proceed if major need for change is discovered mid implementation", "bmad-correct-course", null],
    ],
  }),
  Object.freeze({
    agentCode: "bmad-agent-tech-writer",
    name: "Paige",
    title: "Technical Writer",
    icon: "📚",
    team: "software-development",
    description: "Master of CommonMark, DITA, and OpenAPI; turns complex concepts into accessible structured docs, favors diagrams over walls of text, every word earning its place. Speaks like the patient teacher you wish you'd had, using analogies that make complex things feel simple.",
    relative: "1-analysis/bmad-agent-tech-writer",
    personaHex: "7544242ff68d0ddfd466274f40ae9f3b0ea06ee5f5f938f68f953e19b3598cd7",
    customizationHex: "bb828f2d26a136870099226f07c61297ce88ddd335823b7549592932bbe14a2e",
    menu: [
      ["DP", "Document Project", "Generate comprehensive project documentation (brownfield analysis, architecture scanning)", "bmad-document-project", null],
    ],
  }),
  Object.freeze({
    agentCode: "bmad-agent-ux-designer",
    name: "Sally",
    title: "UX Designer",
    icon: "🎨",
    team: "software-development",
    description: "Balances empathy with edge-case rigor, starts simple and evolves through feedback, every decision serves a genuine user need. Speaks like a filmmaker pitching the scene before the code exists, painting user stories that make you feel the problem.",
    relative: "2-plan-workflows/bmad-agent-ux-designer",
    personaHex: "d780ae59b9fb7e3a94c5ffe4ec75f90942d4a9a3944af4b36c2d279fdd7801db",
    customizationHex: "3df7da44945f9df34dbb3a64de086c756e6c298e73078e388e8f9a4f57cbf7ab",
    menu: [
      ["CU", "Create UX", "Guidance through realizing the plan for your UX to inform architecture and implementation", "bmad-ux", null],
    ],
  }),
]);

const PROMPT_REFERENCES = Object.freeze([
  [
    "WD",
    "Write Document",
    "src/bmm-skills/1-analysis/bmad-agent-tech-writer/write-document.md",
    "c0ddfd981f765b82cba0921dad331cd1fa32bacdeea1f02320edfd60a0ae7e6f",
  ],
  [
    "MG",
    "Generate Mermaid",
    "src/bmm-skills/1-analysis/bmad-agent-tech-writer/mermaid-gen.md",
    "1d83fcc5fa842bc31ecd9fd7e45fbf013fabcadf0022d3391fff5b53b48e4b5d",
  ],
  [
    "VD",
    "Validate Document",
    "src/bmm-skills/1-analysis/bmad-agent-tech-writer/validate-doc.md",
    "3b8d25f60be191716266726393f2d44b77262301b785a801631083b610d6acc5",
  ],
  [
    "EC",
    "Explain Concept",
    "src/bmm-skills/1-analysis/bmad-agent-tech-writer/explain-concept.md",
    "6ea82dbe4e41d4bb8880cbaa62d936e40cef18f8c038be73ae6e09c462abafc9",
  ],
]);

const AGENT_LENSES = Object.freeze([
  "leanness",
  "architecture",
  "determinism",
  "customization",
  "enhancement",
  "agent-cohesion",
]);

function id(prefix, fill = "0") {
  return `${prefix}_01J${fill.repeat(23)}`;
}

function hash(fill) {
  return `sha256:${fill.repeat(64)}`;
}

function exactHash(hex) {
  return `sha256:${hex}`;
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function clone(value) {
  return structuredClone(value);
}

function capability(skillName, normalizedAction = null, moduleCode = "bmm") {
  return {
    packageVersionId: PACKAGE_VERSION_ID,
    moduleCode,
    skillName,
    normalizedAction,
  };
}

function executionProfile({
  actions = [],
  entrypointKind = "direct",
  customizationProfile = "method_skill_toml",
  validationProfile = "MethodOfficialSkillV6",
  resourceTiming = "all_declared_at_start",
  declaredResourcePaths = [],
} = {}) {
  return {
    entrypointKind,
    invocationModes: { interactive: true, headless: false, actions },
    requiredRuntimes: [{ runtime: "node", versionRange: ">=20.12.0", required: true }],
    resourcePolicy: {
      entrypointTiming: "invocation_start",
      resourceTiming,
      declaredResourcePaths,
    },
    declaredToolIntents: [],
    stateHints: [],
    completionEvidence: ["artifact"],
    customizationProfile,
    validationProfile,
    profileHash: hash("2"),
  };
}

function resource(
  path,
  contentHash,
  contentRole = "reference",
  runtimeUse = "descriptive_evidence",
  locationKind = "source_tree",
  treatment = "adapt",
  byteLength = 1,
) {
  return {
    path,
    locationKind,
    contentRole,
    contentHash,
    byteLength,
    treatment,
    runtimeUse,
  };
}

function projectionSource(path, contentHash, treatment = "adapt") {
  return { path, contentHash, treatment };
}

function makeProjection({
  ordinal,
  sourceIdentityHash,
  sourceEntrypoint,
  sourceResources,
  managedPath,
  managedHash,
  blockedToolIntents,
  hostInputReplacements,
}) {
  const projection = {
    projectionId: id("projection", String(ordinal)),
    sourceIdentityHash,
    sourceEntrypoint,
    sourceResources: [...sourceResources].sort((left, right) =>
      left.path < right.path ? -1 : left.path > right.path ? 1 : 0),
    sourceSections: [],
    managedInstruction: {
      path: managedPath,
      contentHash: managedHash,
      format: "SapphirusManagedV1",
    },
    blockedToolIntents,
    hostInputReplacements: hostInputReplacements.map(([toolIntent, inputKind], index) => ({
      toolIntent,
      inputKind,
      inputSchemaHash: hash(String(index + 1)),
    })),
    projectionHash: hash("0"),
  };
  projection.projectionHash = canonicalHash({
    purpose: "bmad-instruction-projection",
    schemaMajor: "v1",
    value: projection,
    excludedFields: ["projectionHash"],
  }).serializedHash;
  return projection;
}

function configGraph(graphKind, moduleCode, skillName, graphHash) {
  const layerKind = graphKind === "compatibility_yaml"
    ? "method_module_yaml"
    : graphKind === "method_central_toml"
      ? "installer_team"
      : "packaged_default";
  return {
    graphKind,
    scope: { packageVersionId: PACKAGE_VERSION_ID, moduleCode, skillName },
    layers: [
      {
        graphKind,
        layerKind,
        ordinal: 0,
        sourcePath: `config/${graphKind}.config`,
        sourceHash: hash("3"),
        entries: [
          {
            key: "extensions.experimental",
            canonicalJson: "true",
            valueHash: hash("4"),
            interpretation: "unknown_untrusted",
          },
        ],
        warnings: [],
      },
    ],
    mergeSemantics: {
      scalarRule: "later_replaces",
      tableRule: "recursive_merge",
      keyedTableArrayRule: "merge_by_code_or_id_when_all_items_keyed",
      otherArrayRule: "append",
      deletionOperator: "none",
    },
    graphHash,
  };
}

function configResolution(graph) {
  return {
    graphKind: graph.graphKind,
    scope: clone(graph.scope),
    graphHash: graph.graphHash,
    orderedLayerHashes: graph.layers.map((layer) => layer.sourceHash),
    resolvedEntries: clone(graph.layers[0].entries),
    warnings: [],
    resolutionHash: hash("5"),
  };
}

function makeDescriptor() {
  const sourceSnapshotHash = hash("e");
  const configGraphs = [
    configGraph("compatibility_yaml", "bmm", null, hash("6")),
    configGraph("method_central_toml", null, null, hash("7")),
    configGraph("skill_customization_toml", "bmm", "bmad-architecture", hash("8")),
  ];
  const helpProjection = makeProjection({
    ordinal: 1,
    sourceIdentityHash: sourceSnapshotHash,
    sourceEntrypoint: projectionSource(
      "src/core-skills/bmad-help/SKILL.md",
      BMAD_HELP_SOURCE_HASH,
      "adapt",
    ),
    sourceResources: [
      projectionSource("src/bmm-skills/module-help.csv", BMM_HELP_HASH, "adopt"),
      projectionSource("src/bmm-skills/module.yaml", MODULE_HASH, "adopt"),
      projectionSource("src/core-skills/module-help.csv", CORE_HELP_HASH, "adopt"),
      projectionSource("src/core-skills/module.yaml", CORE_MODULE_HASH, "adopt"),
    ],
    managedPath: "runtime/method/6.10.0/bmad-help.instructions.md",
    managedHash: MANAGED_HELP_HASH,
    blockedToolIntents: ["file_read", "web"],
    hostInputReplacements: [
      ["file_read", "catalog_snapshot"],
      ["web", "unavailable_fact"],
    ],
  });
  const architectProjection = makeProjection({
    ordinal: 2,
    sourceIdentityHash: sourceSnapshotHash,
    sourceEntrypoint: projectionSource(
      "src/bmm-skills/3-solutioning/bmad-agent-architect/SKILL.md",
      exactHash(AGENTS[1].personaHex),
      "adapt",
    ),
    sourceResources: [
      projectionSource("src/bmm-skills/module.yaml", MODULE_HASH, "adopt"),
      projectionSource(
        "src/bmm-skills/3-solutioning/bmad-agent-architect/customize.toml",
        exactHash(AGENTS[1].customizationHex),
        "adapt",
      ),
    ],
    managedPath: "runtime/method/6.10.0/architect-persona.instructions.md",
    managedHash: MANAGED_ARCHITECT_PERSONA_HASH,
    blockedToolIntents: ["file_read"],
    hostInputReplacements: [["file_read", "resolved_config"]],
  });
  const architectureProjection = makeProjection({
    ordinal: 3,
    sourceIdentityHash: sourceSnapshotHash,
    sourceEntrypoint: projectionSource(
      "src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md",
      ARCHITECTURE_SOURCE_HASH,
      "adapt",
    ),
    sourceResources: [
      projectionSource(
        "src/bmm-skills/3-solutioning/bmad-agent-architect/SKILL.md",
        exactHash(AGENTS[1].personaHex),
        "adapt",
      ),
      projectionSource(
        "src/bmm-skills/3-solutioning/bmad-agent-architect/customize.toml",
        exactHash(AGENTS[1].customizationHex),
        "adapt",
      ),
      projectionSource(
        "src/bmm-skills/3-solutioning/bmad-architecture/customize.toml",
        ARCHITECTURE_CUSTOMIZATION_HASH,
        "adapt",
      ),
      projectionSource(
        "src/bmm-skills/3-solutioning/bmad-architecture/assets/spine-template.md",
        exactHash("ef4ff795624eb5439fae54a06edb389feb5a0cf79cb01ae007af51109335d198"),
        "adapt",
      ),
      projectionSource(
        "src/bmm-skills/3-solutioning/bmad-architecture/references/reviewer-gate.md",
        exactHash("d32e32a3c1d59b5612b947004f3f6fef1117a13ce9f1ffa428d616e0b5d4db69"),
        "adapt",
      ),
      projectionSource(
        "src/bmm-skills/3-solutioning/bmad-architecture/references/headless.md",
        exactHash("4db2708f335c94cb024b5eba022b1e3d88b1ec61c94c4716626d9b89684f5833"),
        "defer",
      ),
      projectionSource(
        "src/bmm-skills/3-solutioning/bmad-create-architecture/SKILL.md",
        exactHash("4b1673582dc8e0957b559cece14f048c0c995874b79ad255f033b36b2b789eb0"),
        "adapt",
      ),
      projectionSource(
        "src/bmm-skills/3-solutioning/bmad-create-architecture/customize.toml",
        exactHash("618be49b0286f18dfb4b6bac3d96bf1cf062994fa5278cd327be9898e651b43c"),
        "adapt",
      ),
      projectionSource(
        "src/bmm-skills/3-solutioning/bmad-architecture/scripts/lint_spine.py",
        exactHash("040e0f1c13d31e6a21c518df943d1d7de7b75d8d98a07603b0983f3f3fb500db"),
        "reject",
      ),
    ],
    managedPath: "runtime/method/6.10.0/architecture-create.instructions.md",
    managedHash: MANAGED_ARCHITECTURE_HASH,
    blockedToolIntents: [
      "external_handoff",
      "file_read",
      "file_write",
      "process",
      "subagent",
      "web",
    ],
    hostInputReplacements: [
      ["external_handoff", "unavailable_fact"],
      ["file_read", "bounded_resource_set"],
      ["file_write", "unavailable_fact"],
      ["process", "unavailable_fact"],
      ["subagent", "unavailable_fact"],
      ["web", "unavailable_fact"],
    ],
  });
  const instructionProjections = [
    helpProjection,
    architectProjection,
    architectureProjection,
  ];
  const resources = [
    ...AGENTS.flatMap(({ relative, personaHex, customizationHex }) => [
      resource(`src/bmm-skills/${relative}/SKILL.md`, exactHash(personaHex), "entrypoint"),
      resource(`src/bmm-skills/${relative}/customize.toml`, exactHash(customizationHex), "config"),
    ]),
    ...PROMPT_REFERENCES.map(([, , path, hex]) =>
      resource(path, exactHash(hex), "reference", "unavailable_reference")),
    resource("src/core-skills/bmad-help/SKILL.md", BMAD_HELP_SOURCE_HASH, "entrypoint"),
    resource("src/core-skills/module.yaml", CORE_MODULE_HASH, "metadata", "descriptive_evidence", "source_tree", "adopt"),
    resource("src/core-skills/module-help.csv", CORE_HELP_HASH, "help_catalog", "reference_data", "source_tree", "adopt"),
    resource("src/bmm-skills/module.yaml", MODULE_HASH, "metadata", "descriptive_evidence", "source_tree", "adopt"),
    resource("src/bmm-skills/module-help.csv", BMM_HELP_HASH, "help_catalog", "reference_data", "source_tree", "adopt"),
    resource("src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md", ARCHITECTURE_SOURCE_HASH, "entrypoint"),
    resource("src/bmm-skills/3-solutioning/bmad-architecture/customize.toml", ARCHITECTURE_CUSTOMIZATION_HASH, "config"),
    resource("src/bmm-skills/3-solutioning/bmad-architecture/assets/spine-template.md", exactHash("ef4ff795624eb5439fae54a06edb389feb5a0cf79cb01ae007af51109335d198"), "asset"),
    resource("src/bmm-skills/3-solutioning/bmad-architecture/references/reviewer-gate.md", exactHash("d32e32a3c1d59b5612b947004f3f6fef1117a13ce9f1ffa428d616e0b5d4db69")),
    resource("src/bmm-skills/3-solutioning/bmad-architecture/references/headless.md", exactHash("4db2708f335c94cb024b5eba022b1e3d88b1ec61c94c4716626d9b89684f5833"), "reference", "unavailable_reference", "source_tree", "defer"),
    resource("src/bmm-skills/3-solutioning/bmad-create-architecture/SKILL.md", exactHash("4b1673582dc8e0957b559cece14f048c0c995874b79ad255f033b36b2b789eb0"), "reference"),
    resource("src/bmm-skills/3-solutioning/bmad-create-architecture/customize.toml", exactHash("618be49b0286f18dfb4b6bac3d96bf1cf062994fa5278cd327be9898e651b43c"), "config"),
    resource("src/bmm-skills/3-solutioning/bmad-architecture/scripts/lint_spine.py", exactHash("040e0f1c13d31e6a21c518df943d1d7de7b75d8d98a07603b0983f3f3fb500db"), "script", "blocked_executable", "source_tree", "reject"),
    resource("runtime/method/6.10.0/bmad-help.instructions.md", MANAGED_HELP_HASH, "managed_instruction", "instruction_data", "managed_projection", "adapt", 1283),
    resource("runtime/method/6.10.0/architect-persona.instructions.md", MANAGED_ARCHITECT_PERSONA_HASH, "managed_instruction", "instruction_data", "managed_projection", "adapt", 899),
    resource("runtime/method/6.10.0/architecture-create.instructions.md", MANAGED_ARCHITECTURE_HASH, "managed_instruction", "instruction_data", "managed_projection", "adapt", 1011),
  ].sort((left, right) => left.path < right.path ? -1 : left.path > right.path ? 1 : 0);
  const architectureProfile = executionProfile({
    actions: ["create"],
    entrypointKind: "step_jit",
    validationProfile: "MethodStepWorkflowV6",
    resourceTiming: "current_step_only",
    declaredResourcePaths: [
      "src/bmm-skills/3-solutioning/bmad-architecture/assets/spine-template.md",
      "src/bmm-skills/3-solutioning/bmad-architecture/references/reviewer-gate.md",
    ],
  });
  const helpProfile = executionProfile({ actions: [] });
  return sealDocument({
    schemaVersion: "sapphirus.bmad-package-descriptor.v1",
    packageVersionId: PACKAGE_VERSION_ID,
    packageName: "bmad-method",
    packageVersion: "6.10.0",
    sourceIdentity: {
      sourceId: "method",
      upstreamLocator: "BMAD-METHOD-main.zip",
      immutableRef: null,
      archiveArtifactLabel: "BMAD-METHOD-main.zip",
      archiveSha256: METHOD_ARCHIVE_HASH,
      sourceTreeHash: sourceSnapshotHash,
      gitIdentity: null,
      packageName: "bmad-method",
      packageVersion: "6.10.0",
      moduleVersion: null,
      sourceFormatVersion: null,
      runtimeCompatibility: [{ runtime: "node", versionRange: ">=20.12.0" }],
      provenanceStatus: "blocked_provenance",
    },
    distributionProfile: "sapphirus_package",
    installProfile: "SapphirusManagedV1",
    metadataOrigin: "synthesized",
    packageMetadataHash: hash("d"),
    sourceSnapshotHash,
    upstreamManifestObservationHashes: [hash("1")],
    finalCompositeInventoryHash: hash("f"),
    modules: [
      {
        moduleCode: "bmm",
        moduleName: "BMad Method",
        moduleVersion: null,
        metadataOrigin: "source",
        metadataSourceHash: MODULE_HASH,
        helpCatalogSourceHash: BMM_HELP_HASH,
        agentRosterSourceHash: MODULE_HASH,
      },
      {
        moduleCode: "core",
        moduleName: "Core",
        moduleVersion: null,
        metadataOrigin: "source",
        metadataSourceHash: CORE_MODULE_HASH,
        helpCatalogSourceHash: CORE_HELP_HASH,
        agentRosterSourceHash: null,
      },
    ],
    instructionProjections,
    resourceInventory: resources,
    skills: [
      {
        moduleCode: "bmm",
        skillName: "bmad-architecture",
        displayName: "Create Architecture",
        description: "Create the architecture spine that keeps independently built units consistent.",
        metadataOrigin: "source",
        sourceEntrypointPath: "src/bmm-skills/3-solutioning/bmad-architecture/SKILL.md",
        sourceEntrypointHash: ARCHITECTURE_SOURCE_HASH,
        resourceSetHash: hash("2"),
        instructionProjectionHash: architectureProjection.projectionHash,
        distributionProfile: "sapphirus_package",
        installProfile: "SapphirusManagedV1",
        executionProfile: architectureProfile,
        skillDescriptorHash: hash("4"),
      },
      {
        moduleCode: "core",
        skillName: "bmad-help",
        displayName: "BMad Help",
        description: "Provide source-grounded Method guidance from the installed catalog.",
        metadataOrigin: "source",
        sourceEntrypointPath: "src/core-skills/bmad-help/SKILL.md",
        sourceEntrypointHash: BMAD_HELP_SOURCE_HASH,
        resourceSetHash: hash("5"),
        instructionProjectionHash: helpProjection.projectionHash,
        distributionProfile: "sapphirus_package",
        installProfile: "SapphirusManagedV1",
        executionProfile: helpProfile,
        skillDescriptorHash: hash("7"),
      },
    ],
    configGraphs,
    configResolutions: configGraphs.map(configResolution),
    validationReportHashes: [],
    descriptorHash: hash("0"),
  });
}

function installedSkill(descriptorSkill, capabilityKeys) {
  const skillName = descriptorSkill.skillName;
  return {
    packageVersionId: PACKAGE_VERSION_ID,
    moduleCode: descriptorSkill.moduleCode,
    skillName,
    actionCardinality: capabilityKeys.length === 1 ? "single_action" : "multi_action",
    capabilityKeys,
    sourceEntrypointHash: descriptorSkill.sourceEntrypointHash,
    resourceSetHash: descriptorSkill.resourceSetHash,
    skillDescriptorHash: descriptorSkill.skillDescriptorHash,
    executionProfileHash: descriptorSkill.executionProfile.profileHash,
    instructionProjectionHash: descriptorSkill.instructionProjectionHash,
    distributionProfile: descriptorSkill.distributionProfile,
    installProfile: descriptorSkill.installProfile,
    entrypointKind: descriptorSkill.executionProfile.entrypointKind,
    validationProfile: descriptorSkill.executionProfile.validationProfile,
    installationObservationHash: hash("6"),
  };
}

function helpAction(capabilityKey, sourceMemberHash, sourceOrdinal, rawRow) {
  return {
    capabilityKey,
    sourceMemberHash,
    sourceRowHash: canonicalHash({
      purpose: "bmad-help-source-row",
      schemaMajor: "v1",
      value: rawRow,
      excludedFields: [],
    }).serializedHash,
    sourceOrdinal,
    rawRow,
    normalized: {
      phase: rawRow.phase || null,
      precededBy: [],
      followedBy: [],
      required: rawRow.required === "true",
      outputLocations: rawRow["output-location"]
        ? rawRow["output-location"].split("|")
        : [],
      outputs: rawRow.outputs ? rawRow.outputs.split("|") : [],
    },
  };
}

function skillMenu(code, displayName, description, ordinal, customizationHash, capabilityKey) {
  const sourceMenuItemHash = canonicalHash({
    purpose: "bmad-agent-menu-item",
    schemaMajor: "v1",
    value: { code, description, capabilityKey },
    excludedFields: [],
  }).serializedHash;
  return {
    menuCode: code,
    displayName,
    description,
    sourceOrdinal: ordinal,
    sourceMenuItemHash,
    target: {
      targetKind: "skill_target",
      sourceCustomizationGraphHash: customizationHash,
      capabilityKey,
    },
  };
}

function makeAgent(agent) {
  const {
    agentCode,
    name,
    title,
    icon,
    team,
    description,
    personaHex,
    customizationHex,
    menu,
  } = agent;
  const customizationHash = exactHash(customizationHex);
  const menuItems = menu.map(([code, displayName, menuDescription, skillName, action], index) =>
    skillMenu(
      code,
      displayName,
      menuDescription,
      index + 1,
      customizationHash,
      capability(skillName, action),
    ));
  if (agentCode === "bmad-agent-tech-writer") {
    const promptDescriptions = [
      "Author a document following documentation best practices through guided conversation",
      "Create a Mermaid-compliant diagram based on your description",
      "Validate documentation against standards and best practices",
      "Create clear technical explanations with examples and diagrams",
    ];
    menuItems.push(...PROMPT_REFERENCES.map(([code, displayName, path, sourceHex], index) => ({
      menuCode: code,
      displayName,
      description: promptDescriptions[index],
      sourceOrdinal: menu.length + index + 1,
      sourceMenuItemHash: canonicalHash({
        purpose: "bmad-agent-prompt-menu-item",
        schemaMajor: "v1",
        value: { code, path, sourceHex },
        excludedFields: [],
      }).serializedHash,
      target: {
        targetKind: "prompt_reference",
        sourceCustomizationGraphHash: customizationHash,
        sourceLocalMemberLabel: path,
        sourceMemberHash: exactHash(sourceHex),
        availability: "unavailable_source_prompt",
      },
    })));
  }
  const agentRecordHash = canonicalHash({
    purpose: "bmad-agent-record",
    schemaMajor: "v1",
    value: {
      moduleCode: "bmm",
      agentCode,
      name,
      title,
      icon,
      team,
      description,
      personaSourceHash: exactHash(personaHex),
      customizationSourceHash: customizationHash,
      menuItems,
    },
    excludedFields: [],
  }).serializedHash;
  return {
    moduleCode: "bmm",
    agentCode,
    name,
    title,
    icon,
    team,
    description,
    moduleSourceHash: MODULE_HASH,
    personaSourceHash: exactHash(personaHex),
    customizationSourceHash: customizationHash,
    personaCustomizationGraphHash: customizationHash,
    menuItems,
    menuGraphHash: canonicalHash({
      purpose: "bmad-agent-menu-graph",
      schemaMajor: "v1",
      value: menuItems,
      excludedFields: [],
    }).serializedHash,
    agentRecordHash,
  };
}

function makeCatalog(descriptor) {
  const architectureCreate = capability("bmad-architecture", "create");
  const help = capability("bmad-help", null, "core");
  const installedKeySet = new Set([
    [architectureCreate.moduleCode, architectureCreate.skillName, architectureCreate.normalizedAction ?? ""].join("\0"),
    [help.moduleCode, help.skillName, help.normalizedAction ?? ""].join("\0"),
  ]);
  const dependencyByKey = new Map();
  for (const agent of AGENTS) {
    for (const [, , , skillName, normalizedAction] of agent.menu) {
      const key = capability(skillName, normalizedAction);
      const encoded = [key.moduleCode, key.skillName, key.normalizedAction ?? ""].join("\0");
      if (!installedKeySet.has(encoded) && !dependencyByKey.has(encoded)) {
        dependencyByKey.set(encoded, {
          capabilityKey: key,
          availability: "unavailable_missing_skill",
          evidenceHash: exactHash(agent.customizationHex),
        });
      }
    }
  }
  const dependencyAvailability = [...dependencyByKey.entries()]
    .sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0)
    .map(([, value]) => value);
  const architectureRow = {
    module: "BMad Method",
    skill: "bmad-architecture",
    "display-name": "Architecture",
    "menu-code": "CA",
    description: "Offer once requirements exist (a PRD or spec; plus UX if present) and the user is ready to move from what to how. Also offer any time independently-built parts risk diverging. Produces the architecture spine: the invariants that keep features epics and stories consistent. Comes before epics and stories and scales from a quick spine to a full architecture (brownfield: ratifies the existing codebase).",
    action: "",
    args: "",
    phase: "3-solutioning",
    "preceded-by": "",
    "followed-by": "",
    required: "true",
    "output-location": "planning_artifacts",
    outputs: "architecture",
  };
  const helpRow = {
    module: "Core",
    skill: "bmad-help",
    "display-name": "BMad Help",
    "menu-code": "BH",
    description: "",
    action: "",
    args: "",
    phase: "anytime",
    "preceded-by": "",
    "followed-by": "",
    required: "false",
    "output-location": "",
    outputs: "",
  };
  const architectureSkill = descriptor.skills.find((skill) => skill.skillName === "bmad-architecture");
  const helpSkill = descriptor.skills.find((skill) => skill.skillName === "bmad-help");
  return sealDocument({
    schemaVersion: "sapphirus.bmad-capability-catalog.v1",
    packageVersionId: PACKAGE_VERSION_ID,
    descriptorHash: descriptor.descriptorHash,
    installedSkills: [
      installedSkill(architectureSkill, [architectureCreate]),
      installedSkill(helpSkill, [help]),
    ],
    helpActionGraph: {
      packageVersionId: PACKAGE_VERSION_ID,
      actions: [
        helpAction(architectureCreate, BMM_HELP_HASH, 19, architectureRow),
        helpAction(help, CORE_HELP_HASH, 4, helpRow),
      ],
      graphHash: hash("c"),
    },
    dependencyAvailability,
    agentRoster: {
      packageVersionId: PACKAGE_VERSION_ID,
      agents: AGENTS.map(makeAgent),
      rosterHash: hash("d"),
    },
    catalogHash: hash("0"),
  });
}

function methodModelBinding() {
  return {
    bindingKind: "method_model",
    providerId: "fixture-provider",
    modelId: "fixture-model",
    deploymentId: "fixture-deployment",
    modelProfileHash: hash("1"),
    modelCapabilityHash: hash("2"),
    contextWindowProfileHash: hash("3"),
    egressProfileHash: hash("4"),
    requestSchemaHash: hash("5"),
    responseSchemaHash: hash("6"),
    bindingHash: hash("7"),
  };
}

function methodConsumption(session, ordinal) {
  return {
    recordKind: "context_decision_consumption",
    consumptionId: id("consume", String(ordinal)),
    decisionId: id("decision", String(ordinal)),
    invocationId: id("invoke", String(ordinal)),
    idempotencyKey: `fixture-method-${ordinal}`,
    sessionId: session.sessionId,
    manifestHash: hash("8"),
    consentHash: hash("9"),
    contextDigest: session.contextDigest,
    packageDescriptorHash: session.packageDescriptorHash,
    packageSourceHash: session.packageSourceHash,
    instructionProjectionHash: session.instructionProjectionHash,
    capabilityCatalogHash: session.capabilityCatalogHash,
    capabilityKey: clone(session.capabilityKey),
    distributionProfile: session.distributionProfile,
    installProfile: session.installProfile,
    executionProfileHash: session.executionProfileHash,
    validationProfileHash: session.validationProfileHash,
    configResolutionHash: session.configResolutionHash,
    customizationHash: session.customizationHash,
    resourceSetHash: session.resourceSetHash,
    modelBinding: clone(session.modelBinding),
    methodSchemaHash: session.methodSchemaHash,
    consumedAt: NOW,
  };
}

function methodCheckpoint(session, ordinal, nextStepKey) {
  return sealDocument({
    schemaVersion: "sapphirus.bmad-method-checkpoint.v1",
    checkpointId: id("checkpoint", String(ordinal)),
    sessionId: session.sessionId,
    turnOrdinal: ordinal,
    capabilityKey: clone(session.capabilityKey),
    capabilityStepTableHash: hash("a"),
    currentStepKey: ordinal === 1 ? "discover" : "decide",
    nextStepKey,
    contextDecisionId: id("decision", String(ordinal)),
    contextDigest: session.contextDigest,
    modelBindingHash: session.modelBinding.bindingHash,
    workingArtifacts: [],
    recordedAt: NOW,
    checkpointHash: hash("0"),
  });
}

function methodReviewEntry(session, ordinal) {
  return {
    entryKind: "accepted_context_review",
    reviewOrdinal: ordinal,
    contextDecisionId: id("decision", String(ordinal)),
    contextDigest: session.contextDigest,
    resourceSetHash: session.resourceSetHash,
    manifestHash: hash("8"),
    consentHash: hash("9"),
    packageDescriptorHash: session.packageDescriptorHash,
    instructionProjectionHash: session.instructionProjectionHash,
    configResolutionHash: session.configResolutionHash,
    customizationHash: session.customizationHash,
    modelBindingHash: session.modelBinding.bindingHash,
    methodSchemaHash: session.methodSchemaHash,
    executionProfileHash: session.executionProfileHash,
    validationProfileHash: session.validationProfileHash,
    reviewedAt: NOW,
    entryHash: hash(String(ordinal)),
  };
}

function makeMethodSession(descriptor, catalog, architect) {
  const descriptorSkill = descriptor.skills.find((skill) =>
    skill.skillName === (architect ? "bmad-architecture" : "bmad-help"));
  const agent = architect
    ? catalog.agentRoster.agents.find((candidate) => candidate.agentCode === "bmad-agent-architect")
    : undefined;
  const menuItem = agent?.menuItems.find((item) => item.menuCode === "CA");
  const session = {
    schemaVersion: "sapphirus.bmad-method-session.v1",
    methodShape: architect ? "architect_iterative" : "no_agent_direct",
    sessionId: id("session", architect ? "2" : "1"),
    deliveryModel: "windows_local",
    authorityRef: {
      authorityKind: "desktop_local_store",
      authorityId: id("authority"),
      installationId: id("install"),
      localStoreId: id("store"),
      authorityEpoch: 1,
    },
    ownerScopeRef: id("ownerscope"),
    projectId: id("project"),
    runId: id("run", architect ? "2" : "1"),
    packageVersionId: PACKAGE_VERSION_ID,
    packageDescriptorHash: descriptor.descriptorHash,
    packageSourceHash: descriptor.sourceSnapshotHash,
    instructionProjectionHash: descriptorSkill.instructionProjectionHash,
    capabilityCatalogHash: catalog.catalogHash,
    agentRosterHash: architect ? catalog.agentRoster.rosterHash : null,
    capabilityKey: architect
      ? capability("bmad-architecture", "create")
      : capability("bmad-help", null, "core"),
    distributionProfile: "sapphirus_package",
    installProfile: "SapphirusManagedV1",
    executionProfile: clone(descriptorSkill.executionProfile),
    executionProfileHash: descriptorSkill.executionProfile.profileHash,
    validationProfile: descriptorSkill.executionProfile.validationProfile,
    validationProfileHash: hash("d"),
    configGraphHash: hash("e"),
    configResolutionHash: hash("f"),
    customizationHash: architect ? ARCHITECTURE_CUSTOMIZATION_HASH : CORE_MODULE_HASH,
    resourceSetHash: descriptorSkill.resourceSetHash,
    contextDigest: hash("3"),
    modelBinding: methodModelBinding(),
    methodSchemaHash: hash("4"),
    artifactExpectations: [],
    agentBinding: architect
      ? {
          bindingKind: "agent",
          rosterHash: catalog.agentRoster.rosterHash,
          moduleSourceHash: MODULE_HASH,
          moduleCode: "bmm",
          agentRecordHash: agent.agentRecordHash,
          agentCode: "bmad-agent-architect",
          agentName: "Winston",
          agentTitle: "System Architect",
          personaHash: MANAGED_ARCHITECT_PERSONA_HASH,
          customizationGraphHash: agent.personaCustomizationGraphHash,
          menuItemHash: menuItem.sourceMenuItemHash,
          menuCode: "CA",
          menuTargetKind: "skill_target",
          menuCapabilityKey: capability("bmad-architecture", "create"),
          agentBindingHash: hash("7"),
        }
      : { bindingKind: "no_agent" },
    contextLedger: {
      ledgerKind: "method_context_ledger",
      ledgerId: id("ledger", architect ? "2" : "1"),
      sessionId: id("session", architect ? "2" : "1"),
      entries: [],
      ledgerHash: hash("8"),
    },
    checkpoints: [],
    decisionConsumptions: [],
    createdAt: NOW,
    contentHash: hash("9"),
  };
  const count = architect ? 2 : 1;
  session.checkpoints = Array.from({ length: count }, (_, index) =>
    methodCheckpoint(session, index + 1, index + 1 === count ? null : "decide"));
  session.decisionConsumptions = Array.from({ length: count }, (_, index) =>
    methodConsumption(session, index + 1));
  session.contextLedger.entries = Array.from({ length: count }, (_, index) =>
    methodReviewEntry(session, index + 1));
  if (architect) {
    session.architectureSpineDrafts = [];
    session.architectureReviewResults = [];
  }
  return sealDocument(session);
}

function makeBuilderDraft(kind) {
  return {
    objectKind: "draft",
    schemaVersion: "sapphirus.bmad-builder-authoring.v1",
    draftId: id(`${kind}draft`),
    ownerScopeRef: id("ownerscope"),
    projectId: id("project"),
    authoringSessionId: id("authorsession"),
    builderKind: kind,
    validationProfile: kind === "agent" ? "BuilderAgentV2Stateless" : "BuilderOutcomeSkillV2",
    authoringAction: {
      builderKind: kind,
      action: kind === "agent" ? "create_rebuild" : "build",
    },
    sourceIdentityHash: hash("1"),
    instructionProjectionSetHash: hash("2"),
    createdAt: NOW,
    draftEffect: "none",
  };
}

function makeBuilderRevision(kind, draft) {
  const files = kind === "agent"
    ? [
        { path: "SKILL.md", content: "# Stateless fixture agent\n" },
        { path: "customize.toml", content: "name = \"fixture\"\n" },
        { path: "references/prompt-quality-canon.md", content: "# Prompt canon\n" },
      ]
    : [{ path: "SKILL.md", content: "# Simple inline fixture workflow\n" }];
  return sealDocument({
    objectKind: "revision",
    schemaVersion: "sapphirus.bmad-builder-revision.v1",
    revisionId: id(`${kind}revision`),
    draftId: draft.draftId,
    builderKind: kind,
    validationProfile: draft.validationProfile,
    authoringAction: clone(draft.authoringAction),
    ordinal: 1,
    parentRevisionHash: null,
    proposedFileSet: { limitProfile: "sapphirus.bmad-builder-limits.v1", files },
    sourceIdentityHash: draft.sourceIdentityHash,
    instructionProjectionSetHash: draft.instructionProjectionSetHash,
    rawResultHash: hash("3"),
    inventoryHash: hash("4"),
    createdAt: NOW,
    revisionHash: hash("0"),
  });
}

function builderModelBinding() {
  return {
    modelHash: hash("5"),
    deploymentHash: hash("6"),
    modelProfileHash: hash("7"),
    schemaHash: hash("8"),
    consentHash: hash("9"),
    contextDecisionId: id("decision", "8"),
    contextDecisionConsumptionHash: hash("a"),
    invocationId: id("invoke", "8"),
    resultHash: hash("b"),
  };
}

function makeBuilderAnalysis(kind, draft, revision, modelLens) {
  const base = {
    objectKind: "analysis",
    schemaVersion: "sapphirus.bmad-builder-analysis.v1",
    analysisId: id(`${kind}analysis`, modelLens ? "2" : "1"),
    draftId: draft.draftId,
    revisionId: revision.revisionId,
    revisionHash: revision.revisionHash,
    builderKind: kind,
    validationProfile: draft.validationProfile,
    analysisKind: modelLens ? "model_lens" : "deterministic_static",
    sourceMemberSetHash: hash("c"),
    instructionProjectionSetHash: draft.instructionProjectionSetHash,
    deterministicFactsHash: hash("d"),
    deterministicFindings: [],
    modelLensesPerformed: modelLens,
    evaluationClaim: "none",
    createdAt: NOW,
    analysisHash: hash("0"),
  };
  if (!modelLens) {
    base.modelLensesNotPerformedReason = "not_requested";
    return sealDocument(base);
  }
  const binding = builderModelBinding();
  base.modelBinding = binding;
  base.modelLensResults = AGENT_LENSES
    .slice(0, kind === "agent" ? 6 : 5)
    .map((lens) => ({
      builderKind: kind,
      lens,
      revisionId: revision.revisionId,
      revisionHash: revision.revisionHash,
      sourceMemberSetHash: base.sourceMemberSetHash,
      instructionProjectionSetHash: base.instructionProjectionSetHash,
      deterministicFactsHash: base.deterministicFactsHash,
      modelHash: binding.modelHash,
      deploymentHash: binding.deploymentHash,
      modelProfileHash: binding.modelProfileHash,
      schemaHash: binding.schemaHash,
      consentHash: binding.consentHash,
      contextDecisionConsumptionHash: binding.contextDecisionConsumptionHash,
      verdict: "clear",
      evaluationClaim: "none",
      findings: [],
    }));
  return sealDocument(base);
}

function makeValidationReport(revision) {
  return sealDocument({
    schemaVersion: "sapphirus.bmad-validation-report.v1",
    reportId: id("report"),
    subjectKind: "builder_draft_revision",
    subjectId: revision.revisionId,
    subjectHash: revision.revisionHash,
    profile: "BuilderAgentV2Stateless",
    findings: [],
    dependencies: [],
    evidence: [],
    disposition: { dispositionKind: "conformant" },
    generatedAt: NOW,
    reportHash: hash("0"),
  });
}

function goldenVector(name, purpose, value, excludedFields) {
  const result = canonicalHash({ purpose, schemaMajor: "v1", value, excludedFields });
  return {
    name,
    purpose,
    schemaMajor: "v1",
    excludedFields,
    value,
    canonicalJson: result.canonicalJson,
    expectedHash: result.serializedHash,
  };
}

export function buildBmadFixtureSet() {
  const descriptor = makeDescriptor();
  const catalog = makeCatalog(descriptor);
  const directSession = makeMethodSession(descriptor, catalog, false);
  const architectSession = makeMethodSession(descriptor, catalog, true);
  const agentDraft = makeBuilderDraft("agent");
  const agentRevision = makeBuilderRevision("agent", agentDraft);
  const agentStatic = makeBuilderAnalysis("agent", agentDraft, agentRevision, false);
  const agentModel = makeBuilderAnalysis("agent", agentDraft, agentRevision, true);
  const workflowDraft = makeBuilderDraft("workflow");
  const workflowRevision = makeBuilderRevision("workflow", workflowDraft);
  const workflowStatic = makeBuilderAnalysis("workflow", workflowDraft, workflowRevision, false);
  const workflowModel = makeBuilderAnalysis("workflow", workflowDraft, workflowRevision, true);
  const validationReport = makeValidationReport(agentRevision);
  const files = new Map();
  const catalogEntries = [];

  const add = (relativePath, value) => files.set(`fixtures/${relativePath}`, stableJson(value));
  const addCatalog = (file, schema, valid, reasonCode = null, contextFile) => {
    const entry = { file, schema, valid, reasonCode };
    if (contextFile !== undefined) entry.contextFile = contextFile;
    catalogEntries.push(entry);
  };
  const addValid = (name, schema, value, contextFile) => {
    const file = `valid/bmad/${name}.json`;
    add(file, value);
    addCatalog(file, schema, true, null, contextFile);
  };
  const addInvalid = (name, schema, value, reasonCode, contextFile) => {
    const file = `invalid/bmad/${name}.json`;
    add(file, value);
    addCatalog(file, schema, false, reasonCode, contextFile);
  };

  addValid("package-descriptor", "bmad-package-descriptor.schema.json", descriptor);
  addValid(
    "capability-catalog",
    "bmad-capability-catalog.schema.json",
    catalog,
    "valid/bmad/package-descriptor.json",
  );
  addValid(
    "method-no-agent-direct",
    "bmad-method-session.schema.json",
    directSession,
    "valid/bmad/capability-catalog.json",
  );
  addValid(
    "method-architect-iterative",
    "bmad-method-session.schema.json",
    architectSession,
    "valid/bmad/capability-catalog.json",
  );
  addValid("builder-agent-draft", "bmad-builder-authoring.schema.json", agentDraft);
  addValid("builder-agent-revision", "bmad-builder-authoring.schema.json", agentRevision);
  addValid("builder-agent-analysis-deterministic", "bmad-builder-authoring.schema.json", agentStatic);
  addValid("builder-agent-analysis-model-lens", "bmad-builder-authoring.schema.json", agentModel);
  addValid("builder-workflow-draft", "bmad-builder-authoring.schema.json", workflowDraft);
  addValid("builder-workflow-revision", "bmad-builder-authoring.schema.json", workflowRevision);
  addValid("builder-workflow-analysis-deterministic", "bmad-builder-authoring.schema.json", workflowStatic);
  addValid("builder-workflow-analysis-model-lens", "bmad-builder-authoring.schema.json", workflowModel);
  addValid("validation-report", "bmad-validation-report.schema.json", validationReport);

  files.set(
    "fixtures/invalid/bmad/duplicate-member.json",
    '{"schemaVersion":"sapphirus.bmad-package-descriptor.v1","schemaVersion":"sapphirus.bmad-package-descriptor.v1"}\n',
  );
  addCatalog("invalid/bmad/duplicate-member.json", null, false, "DUPLICATE_MEMBER");

  const invalidCases = [
    ["unknown-profile", "bmad-builder-authoring.schema.json", agentDraft, (v) => { v.validationProfile = "FutureProfile"; }, "ONE_OF_MISMATCH"],
    ["unknown-action", "bmad-builder-authoring.schema.json", agentRevision, (v) => { v.authoringAction.action = "convert"; }, "ONE_OF_MISMATCH"],
    ["capability-key-collision", "bmad-capability-catalog.schema.json", catalog, (v) => { v.installedSkills[1].capabilityKeys[0] = clone(v.installedSkills[0].capabilityKeys[0]); }, "BMAD_CAPABILITY_KEY_COLLISION"],
    ["installed-skills-unsorted", "bmad-capability-catalog.schema.json", catalog, (v) => { v.installedSkills.reverse(); }, "BMAD_INSTALLED_SKILL_SET_NOT_CANONICAL"],
    ["help-actions-unsorted", "bmad-capability-catalog.schema.json", catalog, (v) => { v.helpActionGraph.actions.reverse(); }, "BMAD_HELP_ACTION_SET_NOT_CANONICAL"],
    ["normalized-action-absent", "bmad-capability-catalog.schema.json", catalog, (v) => { delete v.installedSkills[1].capabilityKeys[0].normalizedAction; }, "REQUIRED_PROPERTY_MISSING"],
    ["help-record-as-installed-skill", "bmad-capability-catalog.schema.json", catalog, (v) => { v.installedSkills[0] = clone(v.helpActionGraph.actions[0]); }, "UNKNOWN_PROPERTY"],
    ["installed-skill-as-help-record", "bmad-capability-catalog.schema.json", catalog, (v) => { v.helpActionGraph.actions[0] = clone(v.installedSkills[0]); }, "UNKNOWN_PROPERTY"],
    ["help-orphan", "bmad-capability-catalog.schema.json", catalog, (v) => { v.helpActionGraph.actions[0].capabilityKey = capability("missing-skill"); }, "BMAD_HELP_ORPHAN"],
    ["dependency-availability-unsorted", "bmad-capability-catalog.schema.json", catalog, (v) => { v.dependencyAvailability.reverse(); }, "BMAD_DEPENDENCY_SET_NOT_CANONICAL"],
    ["agent-roster-unsorted", "bmad-capability-catalog.schema.json", catalog, (v) => { v.agentRoster.agents.reverse(); }, "BMAD_AGENT_ROSTER_NOT_CANONICAL"],
    ["roster-duplicate", "bmad-capability-catalog.schema.json", catalog, (v) => { v.agentRoster.agents.splice(1, 0, clone(v.agentRoster.agents[0])); }, "BMAD_AGENT_ROSTER_NOT_CANONICAL"],
    ["roster-orphan", "bmad-capability-catalog.schema.json", catalog, (v) => { v.agentRoster.agents[0].agentCode = "bmad-agent-orphan"; }, "BMAD_AGENT_ROSTER_NOT_CANONICAL"],
    ["menu-target-mismatch", "bmad-capability-catalog.schema.json", catalog, (v) => { v.agentRoster.agents[4].menuItems[0].target.sourceCustomizationGraphHash = hash("f"); }, "BMAD_MENU_TARGET_TRANSPLANT"],
    ["menu-target-missing-discriminator", "bmad-capability-catalog.schema.json", catalog, (v) => { delete v.agentRoster.agents[4].menuItems[0].target.targetKind; }, "ONE_OF_MISMATCH"],
    ["menu-target-dual-discriminator", "bmad-capability-catalog.schema.json", catalog, (v) => { Object.assign(v.agentRoster.agents[4].menuItems[0].target, { sourceLocalMemberLabel: "x.md", sourceMemberHash: hash("1"), availability: "unavailable_source_prompt" }); }, "ONE_OF_MISMATCH"],
    ["menu-target-unknown-discriminator", "bmad-capability-catalog.schema.json", catalog, (v) => { v.agentRoster.agents[4].menuItems[0].target.targetKind = "future_target"; }, "ONE_OF_MISMATCH"],
    ["prompt-coerced-to-skill", "bmad-capability-catalog.schema.json", catalog, (v) => { v.agentRoster.agents[4].menuItems[1].target.targetKind = "skill_target"; }, "ONE_OF_MISMATCH"],
    ["prompt-member-transplant", "bmad-capability-catalog.schema.json", catalog, (v) => { v.agentRoster.agents[4].menuItems[1].target.sourceMemberHash = hash("f"); }, "BMAD_PROMPT_REFERENCE_TRANSPLANT"],
    ["prompt-label-transplant", "bmad-capability-catalog.schema.json", catalog, (v) => { v.agentRoster.agents[4].menuItems[1].target.sourceLocalMemberLabel = v.agentRoster.agents[4].menuItems[2].target.sourceLocalMemberLabel; }, "BMAD_PROMPT_REFERENCE_TRANSPLANT"],
    ["prompt-label-control-character", "bmad-capability-catalog.schema.json", catalog, (v) => { v.agentRoster.agents[4].menuItems[1].target.sourceLocalMemberLabel = "write\u0001document.md"; }, "ONE_OF_MISMATCH"],
    ["prompt-body-smuggling", "bmad-capability-catalog.schema.json", catalog, (v) => { v.agentRoster.agents[4].menuItems[1].target.body = "execute me"; }, "ONE_OF_MISMATCH"],
    ["persona-hash-substitution", "bmad-capability-catalog.schema.json", catalog, (v) => { v.agentRoster.agents[4].personaSourceHash = hash("f"); }, "BMAD_PERSONA_HASH_MISMATCH"],
    ["same-scope-menu-ambiguity", "bmad-capability-catalog.schema.json", catalog, (v) => { v.agentRoster.agents[4].menuItems[1].menuCode = "DP"; }, "BMAD_MENU_SCOPE_AMBIGUOUS"],
    ["flattened-config-graphs", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.configGraphs.pop(); v.configResolutions.pop(); }, "ARRAY_TOO_SHORT"],
    ["modules-unsorted", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.modules.reverse(); }, "BMAD_MODULE_SET_NOT_CANONICAL"],
    ["skills-unsorted", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.skills.reverse(); }, "BMAD_SKILL_SET_NOT_CANONICAL"],
    ["resources-unsorted", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.resourceInventory.reverse(); }, "BMAD_RESOURCE_SET_NOT_CANONICAL"],
    ["config-graphs-unsorted", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.configGraphs.reverse(); }, "BMAD_CONFIG_GRAPH_NOT_CANONICAL"],
    ["config-resolutions-unsorted", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.configResolutions.reverse(); }, "BMAD_CONFIG_RESOLUTION_NOT_CANONICAL"],
    ["projection-sources-unsorted", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.instructionProjections[0].sourceResources.reverse(); }, "BMAD_INSTRUCTION_PROJECTION_SOURCE_NOT_CANONICAL"],
    ["projection-source-transplant", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.instructionProjections[0].sourceResources[0].contentHash = hash("f"); }, "BMAD_INSTRUCTION_PROJECTION_SOURCE_TRANSPLANT"],
    ["managed-instruction-transplant", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.instructionProjections[0].managedInstruction.contentHash = hash("f"); }, "BMAD_MANAGED_INSTRUCTION_TRANSPLANT"],
    ["skill-projection-transplant", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.skills[0].instructionProjectionHash = v.instructionProjections[0].projectionHash; }, "BMAD_SKILL_PROJECTION_TRANSPLANT"],
    ["method-source-version-transplant", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.packageVersion = "6.0.0"; v.sourceIdentity.packageVersion = "6.0.0"; }, "BMAD_METHOD_SOURCE_IDENTITY_MISMATCH"],
    ["invalid-config-ownership", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.configGraphs[1].scope.moduleCode = "bmm"; }, "BMAD_CONFIG_SCOPE_INVALID"],
    ["generic-workflow-ast", "bmad-builder-authoring.schema.json", workflowRevision, (v) => { v.steps = []; }, "ONE_OF_MISMATCH"],
    ["model-authored-step-key", "bmad-method-session.schema.json", architectSession, (v) => { v.checkpoints[0].currentStepKey = "ModelStep"; }, "ONE_OF_MISMATCH"],
    ["non-monotonic-turn-ordinals", "bmad-method-session.schema.json", architectSession, (v) => { v.checkpoints[1].turnOrdinal = 1; }, "BMAD_TURN_ORDINAL_INVALID"],
    ["reused-context-decision", "bmad-method-session.schema.json", architectSession, (v) => { v.decisionConsumptions[1].decisionId = v.decisionConsumptions[0].decisionId; }, "BMAD_CONTEXT_DECISION_REUSED"],
    ["reused-context-decision-identical-binding", "bmad-method-session.schema.json", architectSession, (v) => { v.decisionConsumptions[1] = clone(v.decisionConsumptions[0]); v.decisionConsumptions[1].invocationId = "invoke_01J00000000000000000000009"; }, "BMAD_CONTEXT_DECISION_REUSED"],
    ["method-authority-transplant", "bmad-method-session.schema.json", directSession, (v) => { v.authorityRef = { authorityKind: "azure_control_plane", authorityId: id("authority"), tenantId: id("tenant"), controlPlaneInstanceId: id("control"), authorityEpoch: 1, region: "westeurope" }; }, "ONE_OF_MISMATCH"],
    ["method-profile-mismatch", "bmad-method-session.schema.json", directSession, (v) => { v.executionProfile.entrypointKind = "inline"; }, "BMAD_HELP_BINDING_MISMATCH"],
    ["method-execution-profile-hash-transplant", "bmad-method-session.schema.json", directSession, (v) => { v.executionProfileHash = hash("f"); }, "BMAD_METHOD_PROFILE_BINDING_MISMATCH"],
    ["method-catalog-hash-transplant", "bmad-method-session.schema.json", architectSession, (v) => { v.capabilityCatalogHash = hash("f"); }, "BMAD_METHOD_CATALOG_BINDING_MISMATCH"],
    ["method-agent-record-transplant", "bmad-method-session.schema.json", architectSession, (v) => { v.agentBinding.agentRecordHash = hash("f"); }, "BMAD_METHOD_AGENT_CATALOG_TRANSPLANT"],
    ["method-menu-item-transplant", "bmad-method-session.schema.json", architectSession, (v) => { v.agentBinding.menuItemHash = hash("f"); }, "BMAD_METHOD_AGENT_CATALOG_TRANSPLANT"],
    ["decision-manifest-transplant", "bmad-method-session.schema.json", architectSession, (v) => { v.decisionConsumptions[0].manifestHash = hash("f"); }, "BMAD_CONTEXT_DECISION_REUSED"],
    ["decision-distribution-transplant", "bmad-method-session.schema.json", architectSession, (v) => { v.decisionConsumptions[0].distributionProfile = "method_source_tree"; }, "BMAD_CONTEXT_DECISION_REUSED"],
    ["decision-model-transplant", "bmad-method-session.schema.json", architectSession, (v) => { v.decisionConsumptions[0].modelBinding.modelProfileHash = hash("f"); }, "BMAD_CONTEXT_DECISION_REUSED"],
    ["agent-build", "bmad-builder-authoring.schema.json", agentRevision, (v) => { v.authoringAction.action = "build"; }, "ONE_OF_MISMATCH"],
    ["convert", "bmad-builder-authoring.schema.json", workflowRevision, (v) => { v.authoringAction.action = "convert"; }, "ONE_OF_MISMATCH"],
    ["analyze-as-evaluation", "bmad-builder-authoring.schema.json", agentModel, (v) => { v.evaluationClaim = "passed"; }, "ONE_OF_MISMATCH"],
    ["future-lifecycle-field", "bmad-builder-authoring.schema.json", agentDraft, (v) => { v.activationState = "active"; }, "ONE_OF_MISMATCH"],
    ["active-builder-draft", "bmad-builder-authoring.schema.json", agentDraft, (v) => { v.draftEffect = "active"; }, "ONE_OF_MISMATCH"],
    ["limit-profile-bypass", "bmad-builder-authoring.schema.json", agentRevision, (v) => { v.proposedFileSet.limitProfile = "unbounded"; }, "ONE_OF_MISMATCH"],
    ["file-cap-bypass", "bmad-builder-authoring.schema.json", agentRevision, (v) => { v.proposedFileSet.files = Array.from({ length: 17 }, (_, i) => ({ path: `references/file-${i}.md`, content: "x" })); }, "ONE_OF_MISMATCH"],
    ["script-authority", "bmad-builder-authoring.schema.json", agentDraft, (v) => { v.command = "node build.js"; }, "ONE_OF_MISMATCH"],
    ["network-authority", "bmad-builder-authoring.schema.json", agentDraft, (v) => { v.remoteEndpoint = "https://example.invalid"; }, "ONE_OF_MISMATCH"],
    ["memory-agent-output", "bmad-builder-authoring.schema.json", agentDraft, (v) => { v.builderKind = "memory_agent"; }, "ONE_OF_MISMATCH"],
    ["autonomous-agent-output", "bmad-builder-authoring.schema.json", agentDraft, (v) => { v.builderKind = "autonomous_agent"; }, "ONE_OF_MISMATCH"],
    ["hash-drift", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.packageName = "transplanted-package"; }, "HASH_MISMATCH"],
    ["package-descriptor-unknown-major", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.schemaVersion = "sapphirus.bmad-package-descriptor.v2"; }, "CONST_MISMATCH"],
    ["capability-catalog-unknown-major", "bmad-capability-catalog.schema.json", catalog, (v) => { v.schemaVersion = "sapphirus.bmad-capability-catalog.v2"; }, "CONST_MISMATCH"],
    ["method-session-unknown-major", "bmad-method-session.schema.json", architectSession, (v) => { v.schemaVersion = "sapphirus.bmad-method-session.v2"; }, "ONE_OF_MISMATCH"],
    ["builder-draft-unknown-major", "bmad-builder-authoring.schema.json", agentDraft, (v) => { v.schemaVersion = "sapphirus.bmad-builder-authoring.v2"; }, "ONE_OF_MISMATCH"],
    ["builder-revision-unknown-major", "bmad-builder-authoring.schema.json", agentRevision, (v) => { v.schemaVersion = "sapphirus.bmad-builder-revision.v2"; }, "ONE_OF_MISMATCH"],
    ["builder-analysis-unknown-major", "bmad-builder-authoring.schema.json", agentModel, (v) => { v.schemaVersion = "sapphirus.bmad-builder-analysis.v2"; }, "ONE_OF_MISMATCH"],
    ["unknown-major", "bmad-validation-report.schema.json", validationReport, (v) => { v.schemaVersion = "sapphirus.bmad-validation-report.v2"; }, "CONST_MISMATCH"],
    ["untrusted-authority-field", "bmad-package-descriptor.schema.json", descriptor, (v) => { v.policyAuthority = true; }, "UNKNOWN_PROPERTY"],
  ];

  for (const [name, schema, source, mutate, reasonCode] of invalidCases) {
    const value = clone(source);
    mutate(value);
    const contextFile = schema === "bmad-capability-catalog.schema.json"
      ? "valid/bmad/package-descriptor.json"
      : schema === "bmad-method-session.schema.json"
        ? "valid/bmad/capability-catalog.json"
        : undefined;
    addInvalid(name, schema, value, reasonCode, contextFile);
  }

  const golden = {
    source: "BMAD-01/canonical-hash-purposes",
    vectors: [
      goldenVector("package-descriptor", "bmad-package-descriptor", descriptor, ["descriptorHash"]),
      goldenVector("capability-catalog", "bmad-capability-catalog", catalog, ["catalogHash"]),
      goldenVector("method-checkpoint", "bmad-method-checkpoint", architectSession.checkpoints[0], ["checkpointHash"]),
      goldenVector("builder-revision", "bmad-builder-revision", agentRevision, ["revisionHash"]),
      goldenVector("builder-analysis", "bmad-builder-analysis", agentModel, ["analysisHash"]),
      goldenVector("validation-report", "bmad-validation-report", validationReport, ["reportHash"]),
    ],
  };
  add("golden/bmad/hash-vectors.json", golden);

  return { files, catalogEntries, golden };
}
