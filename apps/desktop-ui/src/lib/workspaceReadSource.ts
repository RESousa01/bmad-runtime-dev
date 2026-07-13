import type {
  BmadScanProjection,
  ContextPreviewProjection,
  DesktopHostClient,
  WorkspaceEntriesProjection,
  WorkspaceSearchMatch,
  WorkspaceTextProjection,
  WorkspaceTreeEntry,
} from "./hostClient";

export type WorkspaceProjectionProvenance = "local_host" | "browser_demo";

export interface ReadonlyWorkspaceSource {
  provenance: WorkspaceProjectionProvenance;
  listEntries: (cursor: string | null, limit: number) => Promise<WorkspaceEntriesProjection>;
  previewContext: (relativePaths: readonly string[]) => Promise<ContextPreviewProjection>;
  readText: (relativePath: string, maxBytes: number) => Promise<WorkspaceTextProjection>;
  scanBmad: () => Promise<BmadScanProjection>;
  search: (query: string, maxResults: number) => Promise<WorkspaceSearchMatch[]>;
  workspaceId: string;
}

export function createHostWorkspaceSource(
  client: DesktopHostClient,
  workspaceId: string,
): ReadonlyWorkspaceSource {
  return {
    provenance: "local_host",
    workspaceId,
    listEntries: (cursor, limit) => client.listWorkspaceEntries(workspaceId, cursor, limit),
    previewContext: (relativePaths) => client.previewContext(workspaceId, relativePaths),
    readText: (relativePath, maxBytes) =>
      client.readWorkspaceText(workspaceId, relativePath, maxBytes),
    scanBmad: () => client.scanBmad(workspaceId),
    search: (query, maxResults) => client.searchWorkspace(workspaceId, query, maxResults),
  };
}

const demoWorkspaceId = "workspace_browser_demo";
const demoHash = (character: string) => `sha256:${character.repeat(64)}`;

const demoTexts = new Map<string, string>([
  [
    "README.md",
    "# Sapphirus desktop\n\nA governed Windows workspace companion with a narrow, read-only renderer boundary.\n",
  ],
  [
    "package.json",
    "{\n  \"name\": \"sapphirus-desktop\",\n  \"private\": true\n}\n",
  ],
  [
    "src/index.ts",
    "export { scanWorkspace } from \"./workspace/scanner\";\n",
  ],
  [
    "src/workspace/scanner.ts",
    "export function scanWorkspace(entries: readonly string[]) {\n  return entries.filter((entry) => !entry.startsWith(\".git/\"));\n}\n",
  ],
  [
    "tests/workspace/scanner.test.ts",
    "import { scanWorkspace } from \"../../src/workspace/scanner\";\n\nit(\"keeps the scan read only\", () => {\n  expect(scanWorkspace([\"README.md\"])).toEqual([\"README.md\"]);\n});\n",
  ],
  [
    "_bmad/method.yaml",
    "name: sealed-desktop-method\nmode: read-only\n",
  ],
  [
    "_bmad/agents/reviewer.md",
    "# Reviewer\n\nInspect context and produce an inactive planning draft.\n",
  ],
  [
    "_bmad-output/build/workspace-draft.md",
    "# Builder Build draft\n\nInactive in the desktop alpha.\n",
  ],
]);

function demoEntry(
  relativePath: string,
  kind: WorkspaceTreeEntry["kind"],
  childCursor: string | null = null,
): WorkspaceTreeEntry {
  return {
    relativePath,
    kind,
    sizeBytes: demoTexts.has(relativePath)
      ? new TextEncoder().encode(demoTexts.get(relativePath)).byteLength
      : 0,
    childCursor,
  };
}

const demoPages = new Map<string, WorkspaceEntriesProjection>([
  [
    "root",
    {
      workspaceId: demoWorkspaceId,
      entries: [
        demoEntry("_bmad", "directory", "demo_bmad"),
        demoEntry("README.md", "text_file"),
        demoEntry("src", "directory", "demo_src"),
      ],
      nextCursor: "demo_root_next",
    },
  ],
  [
    "demo_root_next",
    {
      workspaceId: demoWorkspaceId,
      entries: [
        demoEntry("_bmad-output", "directory", "demo_bmad_output"),
        demoEntry("package.json", "text_file"),
        demoEntry("tests", "directory", "demo_tests"),
      ],
      nextCursor: null,
    },
  ],
  [
    "demo_src",
    {
      workspaceId: demoWorkspaceId,
      entries: [
        demoEntry("src/index.ts", "text_file"),
        demoEntry("src/workspace", "directory", "demo_src_workspace"),
      ],
      nextCursor: null,
    },
  ],
  [
    "demo_src_workspace",
    {
      workspaceId: demoWorkspaceId,
      entries: [demoEntry("src/workspace/scanner.ts", "text_file")],
      nextCursor: null,
    },
  ],
  [
    "demo_tests",
    {
      workspaceId: demoWorkspaceId,
      entries: [
        demoEntry("tests/workspace", "directory", "demo_tests_workspace"),
      ],
      nextCursor: null,
    },
  ],
  [
    "demo_tests_workspace",
    {
      workspaceId: demoWorkspaceId,
      entries: [demoEntry("tests/workspace/scanner.test.ts", "text_file")],
      nextCursor: null,
    },
  ],
  [
    "demo_bmad",
    {
      workspaceId: demoWorkspaceId,
      entries: [
        demoEntry("_bmad/agents", "directory", "demo_bmad_agents"),
        demoEntry("_bmad/method.yaml", "text_file"),
      ],
      nextCursor: null,
    },
  ],
  [
    "demo_bmad_agents",
    {
      workspaceId: demoWorkspaceId,
      entries: [demoEntry("_bmad/agents/reviewer.md", "text_file")],
      nextCursor: null,
    },
  ],
  [
    "demo_bmad_output",
    {
      workspaceId: demoWorkspaceId,
      entries: [
        demoEntry("_bmad-output/build", "directory", "demo_bmad_output_build"),
      ],
      nextCursor: null,
    },
  ],
  [
    "demo_bmad_output_build",
    {
      workspaceId: demoWorkspaceId,
      entries: [demoEntry("_bmad-output/build/workspace-draft.md", "text_file")],
      nextCursor: null,
    },
  ],
]);

function lineCount(content: string): number {
  const lineFeeds = content.match(/\n/gu)?.length ?? 0;
  return Math.max(1, lineFeeds + (content.endsWith("\n") ? 0 : 1));
}

export const browserDemoWorkspaceSource: ReadonlyWorkspaceSource = {
  provenance: "browser_demo",
  workspaceId: demoWorkspaceId,
  async listEntries(cursor, limit) {
    const page = demoPages.get(cursor ?? "root");
    if (!page) {
      throw new Error("The browser demo page is unavailable.");
    }
    return {
      ...page,
      entries: page.entries.slice(0, Math.max(1, limit)),
    };
  },
  async readText(relativePath, maxBytes) {
    const content = demoTexts.get(relativePath);
    if (content === undefined) {
      throw new Error("The browser demo file is unavailable.");
    }
    const encoded = new TextEncoder().encode(content);
    const visible = new TextDecoder().decode(encoded.slice(0, maxBytes));
    return {
      relativePath,
      content: visible,
      contentHash: demoHash("a"),
      byteCount: encoded.byteLength,
      truncated: encoded.byteLength > maxBytes,
    };
  },
  async search(query, maxResults) {
    const normalized = query.trim().toLocaleLowerCase("en-US");
    const matches: WorkspaceSearchMatch[] = [];
    for (const [relativePath, content] of demoTexts) {
      for (const [index, line] of content.split("\n").entries()) {
        if (line.toLocaleLowerCase("en-US").includes(normalized)) {
          matches.push({ relativePath, line: index + 1, preview: line.trim().slice(0, 240) });
          if (matches.length === maxResults) {
            return matches;
          }
        }
      }
    }
    return matches;
  },
  async scanBmad() {
    return {
      status: "method_and_builder_drafts_detected",
      assets: [
        {
          relativePath: "_bmad/method.yaml",
          assetKind: "method_configuration",
          activation: "read_only",
        },
        {
          relativePath: "_bmad/agents/reviewer.md",
          assetKind: "agent",
          activation: "read_only",
        },
        {
          relativePath: "_bmad-output/build/workspace-draft.md",
          assetKind: "builder_build_draft",
          activation: "inactive_draft",
        },
      ],
      truncated: false,
    };
  },
  async previewContext(relativePaths) {
    const items = relativePaths.map((relativePath) => {
      const content = demoTexts.get(relativePath);
      if (content === undefined) {
        throw new Error("The browser demo context item is unavailable.");
      }
      const byteCount = new TextEncoder().encode(content).byteLength;
      return {
        relativePath,
        startLine: 1 as const,
        endLine: lineCount(content),
        reason: "Selected for this task" as const,
        contentHash: demoHash("a"),
        classification: "source" as const,
        redactions: [] as [],
        byteCount,
        estimatedTokens: Math.floor((byteCount + 3) / 4),
        content,
      };
    });
    return {
      workspaceId: demoWorkspaceId,
      manifestHash: demoHash("c"),
      items,
      totalBytes: items.reduce((sum, item) => sum + item.byteCount, 0),
      estimatedTokens: items.reduce((sum, item) => sum + item.estimatedTokens, 0),
      modelTarget: null,
    };
  },
};
