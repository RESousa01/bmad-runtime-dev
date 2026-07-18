export type PrimaryView = "workspaces" | "agent" | "explorer" | "changes" | "activity";
export type InspectorTab = "context" | "changes" | "logs" | "evidence" | "method";
export type ThemePreference = "dark" | "light" | "system";
export type DensityPreference = "comfortable" | "compact";
export type ProposalState = "ready" | "discarded";

export interface SessionSummary {
  id: string;
  title: string;
  updatedAt: string;
  unread?: boolean;
}

export const initialSessions: SessionSummary[] = [
  { id: "scan", title: "Add a safe workspace scan", updatedAt: "10:42 AM", unread: true },
  { id: "config", title: "Refactor config loader", updatedAt: "9:18 AM", unread: true },
  { id: "parser", title: "Fix test flakiness in parser", updatedAt: "Yesterday" },
  { id: "errors", title: "Improve error messages", updatedAt: "May 19", unread: true },
  { id: "validation", title: "Add validation rules", updatedAt: "May 18" },
];

export const contextItems = [
  {
    path: "src/scan/workspace_scanner.ts",
    range: "Lines 1–226",
    reason: "Implement the bounded workspace scan",
    hash: "sha256:8c2e…9fd1",
    classification: "Source",
    redaction: "No redactions",
    size: "7.9 KB · 1,860 tokens",
  },
  {
    path: "tests/scan/workspace_scanner.test.ts",
    range: "Lines 1–184",
    reason: "Verify exclusions and boundary behavior",
    hash: "sha256:f018…71ac",
    classification: "Source",
    redaction: "No redactions",
    size: "6.1 KB · 1,450 tokens",
  },
  {
    path: ".gitignore",
    range: "Lines 1–38",
    reason: "Respect workspace ignore rules",
    hash: "sha256:2d77…481e",
    classification: "Configuration",
    redaction: "1 secret-like value redacted",
    size: "612 B · 130 tokens",
  },
];

export const evidenceItems = [
  { label: "Proposal constructed", time: "10:42:08", detail: "Candidate hash 1b72…8ee4" },
  { label: "Context reviewed", time: "10:41:56", detail: "3 items · 14.6 KB" },
  { label: "Workspace identity verified", time: "10:41:55", detail: "Grant epoch 7" },
];
