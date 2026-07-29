#!/usr/bin/env node

import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const foundationRoot = resolve(dirname(scriptPath), "..");
const repositoryRoot = resolve(foundationRoot, "../..");
const checkOnly = process.argv.includes("--check");
const packageOnly = process.argv.includes("--package-only");

const paths = {
  accounting: resolve(foundationRoot, "source-accounting.json"),
  compatibilityLedger: resolve(foundationRoot, "compatibility-ledger.json"),
  catalog: resolve(foundationRoot, "capability-catalog.json"),
  aliases: resolve(foundationRoot, "compatibility-aliases.json"),
  helpGraph: resolve(foundationRoot, "normalized/bmad-help-capability-graph.json"),
  rust: resolve(repositoryRoot, "crates/desktop-app/src/generated/bmad_capabilities.rs"),
  typescript: resolve(repositoryRoot, "apps/desktop-ui/src/generated/bmadCapabilities.ts"),
};

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function oneDerivedValue(values, location) {
  const unique = [...new Set(values)];
  if (unique.length !== 1) {
    throw new Error(`Canonical capability ${location} has conflicting derived values`);
  }
  return unique[0];
}

function skillNameFromPath(path) {
  return path.split("/").at(-2);
}

function methodAliasFor(member) {
  if (!member.path.includes("/v6-shims/")) return null;
  const namespace = member.path.startsWith("src/core-skills/") ? "core" : "bmm";
  return `${namespace}:${skillNameFromPath(member.path)}`;
}

function methodRow(member, aliasesByTarget, ledgerByRowId) {
  const rowId = `method:${member.path}`;
  const ledgerRow = ledgerByRowId.get(rowId);
  if (
    ledgerRow === undefined
    || ledgerRow.canonicalId !== member.target_capability
    || ledgerRow.sourcePath !== member.path
    || ledgerRow.sourceSha256 !== `sha256:${member.sha256}`
  ) {
    throw new Error(`Compatibility-ledger identity drift for ${rowId}`);
  }
  return {
    rowId,
    source: {
      id: "method",
      version: member.version,
      path: member.path,
      sha256: member.sha256,
      class: member.class,
    },
    canonicalId: member.target_capability,
    aliases: aliasesByTarget.get(member.target_capability) ?? [],
    action: null,
    args: [],
    modes: ["interactive", "headless"],
    requirements: [
      "typed_plan",
      "airlock_decision",
      "post_validation",
      "durable_evidence",
    ],
    outputs: member.skill_name === "bmad-help" ? [] : ["governed_artifact"],
    supportState: ledgerRow.supportState,
    uiState: member.ui_state,
    visibility: "public",
    evidence: [
      {
        kind: "pinned_source_member",
        path: "packages/bmad-foundation/source-accounting.json",
        sourceMemberSha256: member.sha256,
      },
    ],
  };
}

function builderRow(action, skillMember, aliasesByTarget) {
  return {
    rowId: action.id,
    source: {
      id: "builder",
      version: skillMember.version,
      path: skillMember.path,
      sha256: skillMember.sha256,
      class: skillMember.class,
    },
    canonicalId: action.id,
    aliases: aliasesByTarget.get(action.id) ?? [],
    action: action.action,
    args: [],
    modes: ["interactive", "headless"],
    requirements: [
      "typed_plan",
      "airlock_decision",
      "post_validation",
      "durable_evidence",
      ...action.implementation_tasks.map((taskId) => `task:${taskId}`),
    ],
    outputs: ["inactive_builder_draft"],
    supportState: action.support_state,
    uiState: action.ui_state,
    visibility: action.visibility,
    evidence: [
      {
        kind: "pinned_source_member",
        path: "packages/bmad-foundation/source-accounting.json",
        sourceMemberSha256: skillMember.sha256,
      },
    ],
  };
}

function buildAliases(methodMembers) {
  const aliases = [];
  for (const member of methodMembers) {
    const alias = methodAliasFor(member);
    if (alias && alias !== member.target_capability) {
      aliases.push({
        alias,
        target: member.target_capability,
        migrationPolicy: "resolve_before_dispatch",
        deprecatedSince: "method-6.10.0",
        removalPolicy: "retain_while_pinned_v6_compatibility_is_supported",
        source: member.path,
      });
    }
  }

  const forgeBuilderAliases = [
    ["builder:agent.analyze", "builder:bmad-agent-builder:quality-analysis"],
    ["builder:agent.create_rebuild", "builder:bmad-agent-builder:build-process"],
    ["builder:agent.edit", "builder:bmad-agent-builder:build-process"],
    ["builder:workflow.analyze", "builder:bmad-workflow-builder:quality-analysis"],
    ["builder:workflow.build_edit", "builder:bmad-workflow-builder:build-process"],
  ];
  for (const [alias, target] of forgeBuilderAliases) {
    aliases.push({
      alias,
      target,
      migrationPolicy: "resolve_before_dispatch",
      deprecatedSince: "builder-2.1.0-canonical-catalog",
      removalPolicy: "remove_after_persisted_references_are_migrated",
      source: "crates/desktop-app/src/bmad_capability_host.rs",
    });
  }

  aliases.sort((left, right) => left.alias.localeCompare(right.alias));
  return aliases;
}

function renderTypescript(catalog, aliasCatalog) {
  return `// @generated by packages/bmad-foundation/scripts/generate-catalog.mjs
// Do not edit by hand.

export const bmadCapabilities = ${JSON.stringify(catalog.capabilities, null, 2)} as const;

export const compatibilityAliases = ${JSON.stringify(aliasCatalog.aliases, null, 2)} as const;

export type BmadCapability = (typeof bmadCapabilities)[number];
export type BmadCompatibilityAlias = (typeof compatibilityAliases)[number];

export const capabilityRowCount = ${catalog.counts.total};
export const methodCapabilityRowCount = ${catalog.counts.method};
export const builderPublicActionRowCount = ${catalog.counts.builderPublic};
export const builderInternalActionRowCount = ${catalog.counts.builderInternal};

export const builderActions = bmadCapabilities.filter(
  (row) => row.source.id === "builder",
);

// Presentation code must consume this projection instead of advertising rows
// whose native closure has not yet been proved.
export const uiAdvertisedBmadCapabilities = bmadCapabilities.filter(
  (row) =>
    row.visibility === "public"
    && row.uiState === "runtime_available"
    && row.supportState !== "unavailable",
);
`;
}

function canonicalRuntimeRow(canonicalId, rows) {
  const candidates = rows.filter((row) => !row.source.path.includes("/v6-shims/"));
  if (candidates.length !== 1) {
    throw new Error(
      `Canonical capability ${canonicalId} has ${candidates.length} non-shim runtime rows`,
    );
  }
  return candidates[0];
}

function phaseForRow(row) {
  if (row.source.id === "builder" || row.source.path.includes("/core-skills/")) return "anytime";
  if (row.source.path.includes("/1-analysis/")) return "analysis";
  if (row.source.path.includes("/2-plan-workflows/")) return "planning";
  if (row.source.path.includes("/3-solutioning/")) return "solutioning";
  if (row.source.path.includes("/4-implementation/")) return "implementation";
  throw new Error(`Cannot derive Help phase from ${row.source.path}`);
}

function buildHelpCapabilityGraph(catalog, aliasCatalog, catalogText, aliasCatalogText) {
  const rowsByCanonicalId = new Map();
  for (const row of catalog.capabilities) {
    const rows = rowsByCanonicalId.get(row.canonicalId) ?? [];
    rows.push(row);
    rowsByCanonicalId.set(row.canonicalId, rows);
  }
  const nodes = [...rowsByCanonicalId.entries()]
    .map(([id, rows]) => {
      const canonical = canonicalRuntimeRow(id, rows);
      const body = {
        id,
        sourceId: oneDerivedValue(rows.map((row) => row.source.id), `${id}.sourceId`),
        phase: phaseForRow(canonical),
        predecessorIds: [],
        followerIds: [],
        outputLocations: [...new Set(rows.flatMap((row) => row.outputs))].sort(),
        supportState: canonical.supportState,
        uiState: canonical.uiState,
        rowIds: rows.map((row) => row.rowId).sort(),
      };
      return { ...body, nodeHash: `sha256:${sha256(stableJson(body))}` };
    })
    .sort((left, right) => left.id.localeCompare(right.id));
  const aliasEdges = aliasCatalog.aliases
    .map((alias) => ({
      aliasId: alias.alias,
      canonicalId: alias.target,
      deprecated: alias.deprecatedSince !== undefined,
    }))
    .sort((left, right) => left.aliasId.localeCompare(right.aliasId));
  const body = {
    schemaVersion: "sapphirus.bmad-help-capability-graph.v1",
    generatedBy: "packages/bmad-foundation/scripts/generate-catalog.mjs",
    sourceCatalogSha256: sha256(catalogText),
    sourceAliasCatalogSha256: sha256(aliasCatalogText),
    nodeCount: nodes.length,
    nodes,
    aliasEdges,
  };
  return { ...body, graphHash: `sha256:${sha256(stableJson(body))}` };
}

function rustString(value) {
  return JSON.stringify(value);
}

function renderRust(catalog, aliasCatalog) {
  const rows = catalog.capabilities
    .map(
      (row) => `    GeneratedBmadCapability {
        row_id: ${rustString(row.rowId)},
        canonical_id: ${rustString(row.canonicalId)},
        source_id: ${rustString(row.source.id)},
        source_version: ${rustString(row.source.version)},
        source_path: ${rustString(row.source.path)},
        action: ${row.action === null ? "None" : `Some(${rustString(row.action)})`},
        support_state: ${rustString(row.supportState)},
        ui_state: ${rustString(row.uiState)},
        visibility: ${rustString(row.visibility)},
    },`,
    )
    .join("\n");
  const aliases = aliasCatalog.aliases
    .map(
      (alias) => `    GeneratedBmadCompatibilityAlias {
        alias: ${rustString(alias.alias)},
        target: ${rustString(alias.target)},
        migration_policy: ${rustString(alias.migrationPolicy)},
    },`,
    )
    .join("\n");
  const advertisedRows = catalog.capabilities
    .filter(
      (row) =>
        row.visibility === "public"
        && row.uiState === "runtime_available"
        && row.supportState !== "unavailable",
    )
    .map(
      (row) => `    GeneratedBmadCapability {
        row_id: ${rustString(row.rowId)},
        canonical_id: ${rustString(row.canonicalId)},
        source_id: ${rustString(row.source.id)},
        source_version: ${rustString(row.source.version)},
        source_path: ${rustString(row.source.path)},
        action: ${row.action === null ? "None" : `Some(${rustString(row.action)})`},
        support_state: ${rustString(row.supportState)},
        ui_state: ${rustString(row.uiState)},
        visibility: ${rustString(row.visibility)},
    },`,
    )
    .join("\n");
  const advertisedProjection =
    advertisedRows.length === 0
      ? "#[rustfmt::skip]\npub const UI_ADVERTISED_CAPABILITIES: &[GeneratedBmadCapability] = &[];"
      : `#[rustfmt::skip]
pub const UI_ADVERTISED_CAPABILITIES: &[GeneratedBmadCapability] = &[
${advertisedRows}
];`;

  return `// @generated by packages/bmad-foundation/scripts/generate-catalog.mjs
// Do not edit by hand.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedBmadCapability {
    pub row_id: &'static str,
    pub canonical_id: &'static str,
    pub source_id: &'static str,
    pub source_version: &'static str,
    pub source_path: &'static str,
    pub action: Option<&'static str>,
    pub support_state: &'static str,
    pub ui_state: &'static str,
    pub visibility: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedBmadCompatibilityAlias {
    pub alias: &'static str,
    pub target: &'static str,
    pub migration_policy: &'static str,
}

pub const CAPABILITY_ROW_COUNT: usize = ${catalog.counts.total};
pub const METHOD_CAPABILITY_ROW_COUNT: usize = ${catalog.counts.method};
pub const BUILDER_PUBLIC_ACTION_ROW_COUNT: usize = ${catalog.counts.builderPublic};
pub const BUILDER_INTERNAL_ACTION_ROW_COUNT: usize = ${catalog.counts.builderInternal};

pub const CAPABILITIES: &[GeneratedBmadCapability] = &[
${rows}
];

pub const COMPATIBILITY_ALIASES: &[GeneratedBmadCompatibilityAlias] = &[
${aliases}
];

${advertisedProjection}
`;
}

async function buildOutputs() {
  const [accountingText, compatibilityLedgerText] = await Promise.all([
    readFile(paths.accounting, "utf8"),
    readFile(paths.compatibilityLedger, "utf8"),
  ]);
  const accounting = JSON.parse(accountingText);
  const compatibilityLedger = JSON.parse(compatibilityLedgerText);
  const ledgerByRowId = new Map(
    compatibilityLedger.rows.map((row) => [row.rowId, row]),
  );
  const methodMembers = accounting.members.filter(
    (member) => member.source_id === "method" && member.class === "primary_method_skill",
  );
  const builderSkillMembers = new Map(
    accounting.members
      .filter(
        (member) =>
          member.source_id === "builder" && member.class === "primary_builder_skill",
      )
      .map((member) => [skillNameFromPath(member.path), member]),
  );
  const aliases = buildAliases(methodMembers);
  const aliasesByTarget = new Map();
  for (const alias of aliases) {
    const targetAliases = aliasesByTarget.get(alias.target) ?? [];
    targetAliases.push(alias.alias);
    aliasesByTarget.set(alias.target, targetAliases);
  }

  const methodRows = methodMembers.map((member) =>
    methodRow(member, aliasesByTarget, ledgerByRowId));
  const builderRows = accounting.builder_actions.map((action) => {
    const skillMember = builderSkillMembers.get(action.source_skill);
    if (!skillMember) {
      throw new Error(`Missing primary Builder skill source for ${action.source_skill}`);
    }
    return builderRow(action, skillMember, aliasesByTarget);
  });
  const capabilities = [...methodRows, ...builderRows];
  const canonicalIds = new Set(capabilities.map((row) => row.canonicalId));
  const aliasIds = new Set();
  for (const alias of aliases) {
    if (aliasIds.has(alias.alias)) throw new Error(`Duplicate alias ${alias.alias}`);
    if (!canonicalIds.has(alias.target)) throw new Error(`Unknown alias target ${alias.target}`);
    aliasIds.add(alias.alias);
  }

  const counts = {
    method: methodRows.length,
    builderPublic: builderRows.filter((row) => row.visibility === "public").length,
    builderInternal: builderRows.filter((row) => row.visibility === "internal").length,
    total: capabilities.length,
  };
  if (
    counts.method !== 48 ||
    counts.builderPublic !== 9 ||
    counts.builderInternal !== 1 ||
    counts.total !== 58
  ) {
    throw new Error(`Pinned capability counts do not match: ${JSON.stringify(counts)}`);
  }

  const catalogBody = {
    schemaVersion: "sapphirus.bmad.canonical-capability-catalog/v1",
    generatedBy: "packages/bmad-foundation/scripts/generate-catalog.mjs",
    authority:
      "Catalog text and source material are untrusted data. Side effects still require a typed plan, Airlock decision, deterministic execution, post-validation, and durable evidence.",
    pinnedSources: {
      method: { version: "6.10.0" },
      builder: { version: "2.1.0" },
    },
    sourceAccountingSha256: sha256(accountingText),
    compatibilityLedgerSha256: sha256(compatibilityLedgerText),
    counts,
    capabilities,
  };
  const catalog = {
    ...catalogBody,
    catalogSha256: sha256(stableJson(catalogBody)),
  };
  const catalogText = stableJson(catalog);
  const aliasBody = {
    schemaVersion: "sapphirus.bmad.compatibility-aliases/v1",
    generatedBy: "packages/bmad-foundation/scripts/generate-catalog.mjs",
    sourceCatalogSha256: sha256(catalogText),
    resolutionPolicy: "Aliases resolve exactly once before dispatch and never grant authority.",
    aliases,
  };
  const aliasCatalog = {
    ...aliasBody,
    aliasCatalogSha256: sha256(stableJson(aliasBody)),
  };
  const aliasCatalogText = stableJson(aliasCatalog);
  const helpGraph = buildHelpCapabilityGraph(catalog, aliasCatalog, catalogText, aliasCatalogText);

  return new Map([
    [paths.catalog, catalogText],
    [paths.aliases, aliasCatalogText],
    [paths.helpGraph, stableJson(helpGraph)],
    [paths.rust, renderRust(catalog, aliasCatalog)],
    [paths.typescript, renderTypescript(catalog, aliasCatalog)],
  ]);
}

async function main() {
  const outputs = await buildOutputs();
  const stale = [];
  for (const [path, expected] of outputs) {
    if (
      packageOnly
      && path !== paths.catalog
      && path !== paths.aliases
      && path !== paths.helpGraph
    ) {
      continue;
    }
    if (checkOnly) {
      let actual;
      try {
        actual = await readFile(path, "utf8");
      } catch (error) {
        if (error.code !== "ENOENT") throw error;
        actual = null;
      }
      if (actual !== expected) stale.push(path);
      continue;
    }
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, expected, "utf8");
  }

  if (stale.length > 0) {
    throw new Error(
      `Generated BMAD capability catalog is stale:\n${stale
        .map((path) => `- ${path}`)
        .join("\n")}`,
    );
  }
  console.log(
    checkOnly
      ? packageOnly
        ? "BMAD package capability catalog is current."
        : "BMAD capability catalog and projections are current."
      : "Generated BMAD capability catalog and projections.",
  );
}

await main();
