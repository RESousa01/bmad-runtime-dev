import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import test from "node:test";

const root = process.cwd();

async function source(...parts) {
  return readFile(join(root, ...parts), "utf8");
}

test("reviewed recovery commands stay separated and closed at every executable boundary", async () => {
  const [runtime, ipc, host, guard] = await Promise.all([
    source("crates", "desktop-runtime", "src", "command.rs"),
    source("crates", "desktop-ipc", "src", "envelope.rs"),
    source("crates", "desktop-app", "src", "commands.rs"),
    source("tools", "check-boundaries.mjs"),
  ]);

  for (const command of ["changes.recovery.prepare", "changes.recovery.decide"]) {
    assert.match(runtime, new RegExp(`=> "${command.replaceAll(".", "\\.")}"`));
    assert.match(ipc, new RegExp(`"${command.replaceAll(".", "\\.")}"`));
    assert.match(host, new RegExp(`"${command.replaceAll(".", "\\.")}"`));
    assert.match(guard, new RegExp(`"${command.replaceAll(".", "\\.")}"`));
  }
  assert.match(host, /LocalCommand::PrepareChangesRecovery[\s\S]*LocalCommand::DecideChangesRecovery/u);
  assert.match(host, /crate::recovery::prepare_recovery\([\s\S]*renderer_session/u);
  assert.match(host, /crate::recovery::decide_recovery\([\s\S]*renderer_session/u);
  assert.doesNotMatch(ipc, /struct (?:Prepare|Decide)ChangesRecoveryPayload[\s\S]{0,500}(?:absolute_path|shell|checkpoint_content|provider)/u);
});

test("reviewed catalog and update blocking remain executable guard invariants", async () => {
  const guard = await source("tools", "check-boundaries.mjs");
  assert.match(guard, /reviewedReadyCommands[\s\S]*"changes\.history",\s*"changes\.recovery\.prepare",\s*"changes\.recovery\.decide"/u);
  assert.match(guard, /recoveryExcludedFromReplyCache/u);
  assert.match(guard, /recovery_required[\s\S]*restoring[\s\S]*manual_review[\s\S]*update/u);
});
