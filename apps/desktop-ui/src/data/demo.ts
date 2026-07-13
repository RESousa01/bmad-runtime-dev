export type PrimaryView = "workspaces" | "agent" | "explorer" | "changes" | "activity";
export type InspectorTab = "context" | "changes" | "logs" | "evidence";
export type ThemePreference = "dark" | "light" | "system";
export type DensityPreference = "comfortable" | "compact";
export type ProposalState = "ready" | "discarded";

export interface SessionSummary {
  id: string;
  title: string;
  updatedAt: string;
  unread?: boolean;
}

export interface DiffLine {
  kind: "context" | "added" | "removed";
  newNumber?: number;
  oldNumber?: number;
  text: string;
}

export const initialSessions: SessionSummary[] = [
  { id: "scan", title: "Add a safe workspace scan", updatedAt: "10:42 AM", unread: true },
  { id: "config", title: "Refactor config loader", updatedAt: "9:18 AM", unread: true },
  { id: "parser", title: "Fix test flakiness in parser", updatedAt: "Yesterday" },
  { id: "errors", title: "Improve error messages", updatedAt: "May 19", unread: true },
  { id: "validation", title: "Add validation rules", updatedAt: "May 18" },
];

export const diffLines: DiffLine[] = [
  { kind: "context", oldNumber: 1, newNumber: 1, text: "import { promises as fs } from 'fs';" },
  { kind: "added", newNumber: 2, text: "import path from 'path';" },
  { kind: "added", newNumber: 3, text: "import ignore from 'ignore';" },
  { kind: "context", oldNumber: 2, newNumber: 4, text: "" },
  { kind: "added", newNumber: 5, text: "export interface ScanOptions {" },
  { kind: "added", newNumber: 6, text: "  root: string;" },
  { kind: "added", newNumber: 7, text: "  maxFileSizeBytes?: number;" },
  { kind: "added", newNumber: 8, text: "  followSymlinks?: boolean;" },
  { kind: "added", newNumber: 9, text: "}" },
  { kind: "context", oldNumber: 3, newNumber: 10, text: "" },
  { kind: "removed", oldNumber: 4, text: "export interface ScanResult { files: string[]; }" },
  { kind: "added", newNumber: 11, text: "export interface ScanResult {" },
  { kind: "added", newNumber: 12, text: "  files: string[];" },
  { kind: "added", newNumber: 13, text: "  totalBytes: number;" },
  { kind: "added", newNumber: 14, text: "  truncated: boolean;" },
  { kind: "added", newNumber: 15, text: "  ignoredCount: number;" },
  { kind: "added", newNumber: 16, text: "}" },
  { kind: "context", oldNumber: 210, newNumber: 220, text: "if (lst.isDirectory()) {" },
  { kind: "context", oldNumber: 211, newNumber: 221, text: "  return;" },
  { kind: "context", oldNumber: 212, newNumber: 222, text: "}" },
  { kind: "removed", oldNumber: 213, text: "const entries = await fs.readdir(dir);" },
  { kind: "added", newNumber: 223, text: "const entries = await fs.readdir(dir, { withFileTypes: true });" },
  { kind: "context", oldNumber: 214, newNumber: 224, text: "for (const entry of entries) {" },
  { kind: "context", oldNumber: 215, newNumber: 225, text: "  const full = path.join(dir, entry.name);" },
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
