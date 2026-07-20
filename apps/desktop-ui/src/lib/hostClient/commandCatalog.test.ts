import { describe, expect, it } from "vitest";
import { desktopHostCommands } from "./contracts";

// Tripwire ahead of tools/check-boundaries.mjs: the renderer catalog must
// mirror the host READY_COMMANDS projection exactly, in order.
describe("desktop host command catalog", () => {
  it("carries the reviewed 33-command catalog in reviewed order", () => {
    expect(desktopHostCommands).toEqual([
      "app.get_boot_state",
      "workspace.select_folder",
      "workspace.list",
      "workspace.revoke",
      "workspace.list_entries",
      "workspace.read_text",
      "workspace.search",
      "bmad.scan",
      "bmad.library.snapshot",
      "bmad.persona.view",
      "model.auth.status",
      "model.auth.sign_in",
      "model.auth.sign_out",
      "bmad.help.prepare",
      "bmad.help.approve",
      "bmad.help.cancel",
      "bmad.help.submit",
      "bmad.help.latest",
      "run.create",
      "context.preview",
      "workspace.enable_edits",
      "changes.propose",
      "approval.decide",
      "rollback.request",
      "changes.history",
      "changes.recovery.prepare",
      "changes.recovery.decide",
      "app.preferences.get",
      "app.preferences.set",
      "app.about",
      "app.offboarding.inspect",
      "app.offboarding.erase",
      "workspace.pick_files",
    ]);
  });
});
