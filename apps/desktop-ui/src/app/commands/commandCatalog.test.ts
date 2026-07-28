import { describe, expect, it, vi } from "vitest";
import {
  commandDefinitions,
  createCommandRegistry,
  matchesQuery,
  type CommandActions,
  type CommandContext,
} from "./commands";
import { CommandRegistry, unavailable } from "./commandRegistry";
import {
  commandForEvent,
  findKeybindingConflicts,
  formatKeybinding,
  matchesChord,
  parseChord,
} from "./keybindings";

function createActions(): { actions: CommandActions; ran: string[] } {
  const ran: string[] = [];
  const actions: CommandActions = {
    startNewSession: () => ran.push("startNewSession"),
    selectSession: (id) => ran.push(`selectSession:${id}`),
    closeTask: (id) => ran.push(`closeTask:${id}`),
    requestTaskDeletion: (id) => ran.push(`requestTaskDeletion:${id}`),
    openWorkspaceManager: () => ran.push("openWorkspaceManager"),
    openContextDrawer: (kind) => ran.push(`openContextDrawer:${kind}`),
    openSettings: () => ran.push("openSettings"),
    togglePalette: () => ran.push("togglePalette"),
  };
  return { actions, ran };
}

function context(overrides: Partial<CommandContext> = {}): CommandContext {
  const { actions } = createActions();
  return {
    selectedTaskTitle: null,
    selectedTaskId: null,
    workspaceAvailable: true,
    actions,
    ...overrides,
  };
}

/** Every context shape the availability rules distinguish between. */
const contexts: readonly CommandContext[] = [
  context(),
  context({ selectedTaskId: "t1", selectedTaskTitle: "Fix login" }),
  context({ workspaceAvailable: false }),
  context({
    selectedTaskId: "t1",
    selectedTaskTitle: "Fix login",
    workspaceAvailable: false,
  }),
];

describe("the catalog", () => {
  it("registers every command under a unique id", () => {
    // The registry throws on a duplicate; building it is the assertion.
    expect(() => createCommandRegistry(context())).not.toThrow();
  });

  it("offers the same set of commands whatever the context", () => {
    // FE-09 acceptance: the same command is reachable from every entry point.
    // Dropping a command from the list when it cannot run is how the palette
    // used to behave, and it is what makes "unavailable" indistinguishable
    // from "does not exist".
    const ids = contexts.map((ctx) => commandDefinitions(ctx).map((d) => d.id));
    for (const set of ids) expect(set).toEqual(ids[0]);
  });

  it("gives every unavailable command a user-readable reason", () => {
    // FE-09 acceptance, checked across the whole catalog rather than on the
    // one command whose rule someone happened to write a test for.
    for (const ctx of contexts) {
      for (const resolved of createCommandRegistry(ctx).resolveAll(ctx)) {
        if (resolved.availability.kind !== "unavailable") continue;
        const { reason } = resolved.availability;
        expect(reason.length, `${resolved.definition.id} has an empty reason`).toBeGreaterThan(0);
        // A reason is a sentence for a person, not a state dump. These are the
        // shapes that keep leaking in: an enum name, or a bare negation.
        expect(reason, `${resolved.definition.id}`).toMatch(/^[A-Z].*[.?]$/u);
        expect(reason).not.toMatch(/^(null|undefined|false|error)/iu);
      }
    }
  });

  it("names the open task in the title of task commands", () => {
    const withTask = commandDefinitions(
      context({ selectedTaskId: "t1", selectedTaskTitle: "Fix login" }),
    );
    expect(withTask.find((d) => d.id === "task.close")?.title).toBe("Close task: Fix login");
    const without = commandDefinitions(context());
    expect(without.find((d) => d.id === "task.close")?.title).toBe("Close task");
  });
});

describe("availability", () => {
  it("blocks task commands with no task open, and says why", () => {
    const ctx = context();
    const registry = createCommandRegistry(ctx);
    for (const id of ["task.focus", "task.close", "task.delete"]) {
      expect(registry.resolve(id, ctx)?.availability).toEqual(
        unavailable("Open a task first."),
      );
    }
  });

  it("allows task commands once a task is open", () => {
    const ctx = context({ selectedTaskId: "t1", selectedTaskTitle: "Fix login" });
    const registry = createCommandRegistry(ctx);
    for (const id of ["task.focus", "task.close", "task.delete"]) {
      expect(registry.resolve(id, ctx)?.availability.kind).toBe("available");
    }
  });

  it("blocks the workspace command in the browser preview", () => {
    const ctx = context({ workspaceAvailable: false });
    const resolved = createCommandRegistry(ctx).resolve("workspace.open", ctx);
    expect(resolved?.availability.kind).toBe("unavailable");
  });

  it("leaves starting a task available with no model provider connected", () => {
    // Guards a regression the registry made easy to introduce: gating this on
    // the composer's send condition would have removed something the app
    // allowed before the catalog existed. Only sending needs a provider.
    const ctx = context();
    expect(createCommandRegistry(ctx).resolve("task.new", ctx)?.availability.kind)
      .toBe("available");
  });
});

describe("invoke", () => {
  it("runs an available command", async () => {
    const { actions, ran } = createActions();
    const ctx = context({ actions });
    const registry = createCommandRegistry(ctx);
    expect(await registry.invoke("view.files", ctx, "palette")).toEqual({ kind: "ran" });
    expect(ran).toEqual(["openContextDrawer:files"]);
  });

  it("refuses an unavailable command and returns the reason", async () => {
    const { actions, ran } = createActions();
    const ctx = context({ actions });
    const registry = createCommandRegistry(ctx);
    const outcome = await registry.invoke("task.close", ctx, "keybinding");
    expect(outcome).toEqual({ kind: "unavailable", reason: "Open a task first." });
    // The refusal is in the registry, so a surface that forgot to grey its
    // button still cannot run the command.
    expect(ran).toEqual([]);
  });

  it("reports an unknown id rather than throwing", async () => {
    const ctx = context();
    expect(await createCommandRegistry(ctx).invoke("nope", ctx, "menu")).toEqual({
      kind: "unknown",
    });
  });

  it("produces the same result from every entry point", async () => {
    // FE-09 acceptance, stated as directly as it can be: palette, menu,
    // keybinding and toolbar differ only in the `source` they record.
    for (const source of ["palette", "menu", "keybinding", "toolbar"] as const) {
      const { actions, ran } = createActions();
      const ctx = context({ actions, selectedTaskId: "t1", selectedTaskTitle: "Fix login" });
      const registry = createCommandRegistry(ctx);
      expect(await registry.invoke("task.close", ctx, source)).toEqual({ kind: "ran" });
      expect(ran).toEqual(["closeTask:t1"]);
    }
  });

  it("passes the source through to the command", async () => {
    const seen: string[] = [];
    const registry = new CommandRegistry<CommandContext>([
      {
        id: "probe",
        title: "Probe",
        category: "Test",
        availability: () => ({ kind: "available" }),
        run: (_ctx, source) => { seen.push(source); },
      },
    ]);
    const ctx = context();
    await registry.invoke("probe", ctx, "toolbar");
    expect(seen).toEqual(["toolbar"]);
  });

  it("awaits an async command before reporting that it ran", async () => {
    const order: string[] = [];
    const registry = new CommandRegistry<CommandContext>([
      {
        id: "slow",
        title: "Slow",
        category: "Test",
        availability: () => ({ kind: "available" }),
        run: async () => {
          await Promise.resolve();
          order.push("finished");
        },
      },
    ]);
    await registry.invoke("slow", context(), "palette");
    order.push("reported");
    expect(order).toEqual(["finished", "reported"]);
  });
});

describe("registry", () => {
  it("rejects a duplicate id", () => {
    const registry = new CommandRegistry<CommandContext>();
    const definition = {
      id: "dup",
      title: "Dup",
      category: "Test",
      availability: () => ({ kind: "available" as const }),
      run: vi.fn(),
    };
    registry.register(definition);
    expect(() => registry.register(definition)).toThrow(/already registered/u);
  });
});

describe("matchesQuery", () => {
  const definitions = commandDefinitions(context());
  const byId = (id: string) => definitions.find((d) => d.id === id)!;

  it("matches on the title", () => {
    expect(matchesQuery(byId("app.settings"), "settings")).toBe(true);
  });

  it("matches on a keyword the title does not contain", () => {
    // "preferences" is what a user coming from another editor types.
    expect(matchesQuery(byId("app.settings"), "preferences")).toBe(true);
  });

  it("matches on the category", () => {
    expect(matchesQuery(byId("view.files"), "view")).toBe(true);
  });

  it("does not match unrelated text", () => {
    expect(matchesQuery(byId("app.settings"), "zzz")).toBe(false);
  });

  it("matches everything for an empty query", () => {
    expect(definitions.every((d) => matchesQuery(d, "  "))).toBe(true);
  });
});

describe("keybindings", () => {
  it("parses a plain chord", () => {
    expect(parseChord("Mod+K")).toEqual({ key: "k", mod: true, shift: false, alt: false });
  });

  it("parses every modifier", () => {
    expect(parseChord("Mod+Shift+Alt+P")).toEqual({
      key: "p", mod: true, shift: true, alt: true,
    });
  });

  it("rejects an unknown modifier", () => {
    expect(parseChord("Hyper+K")).toBeNull();
    expect(parseChord("")).toBeNull();
  });

  it("resolves Mod to Cmd on macOS and Ctrl elsewhere", () => {
    const chord = parseChord("Mod+K")!;
    const base = { key: "k", shiftKey: false, altKey: false };
    expect(matchesChord({ ...base, metaKey: true, ctrlKey: false }, chord, true)).toBe(true);
    expect(matchesChord({ ...base, metaKey: false, ctrlKey: true }, chord, true)).toBe(false);
    expect(matchesChord({ ...base, metaKey: false, ctrlKey: true }, chord, false)).toBe(true);
    expect(matchesChord({ ...base, metaKey: true, ctrlKey: false }, chord, false)).toBe(false);
  });

  it("does not fire when an extra modifier is held", () => {
    // Mod+Shift+K is a different chord from Mod+K, and a user who has bound
    // both must not get the wrong one.
    const chord = parseChord("Mod+K")!;
    expect(matchesChord(
      { key: "k", ctrlKey: true, metaKey: false, shiftKey: true, altKey: false },
      chord,
      false,
    )).toBe(false);
  });

  it("labels a chord in the conventions of the platform", () => {
    expect(formatKeybinding("Mod+K", false)).toBe("Ctrl+K");
    expect(formatKeybinding("Mod+K", true)).toBe("⌘K");
    expect(formatKeybinding("Mod+Shift+P", false)).toBe("Ctrl+Shift+P");
    expect(formatKeybinding("Mod+Enter", false)).toBe("Ctrl+Enter");
    expect(formatKeybinding("Mod+Enter", true)).toBe("⌘⏎");
  });

  it("leaves an unparseable binding alone rather than rendering nonsense", () => {
    expect(formatKeybinding("Hyper+K", false)).toBe("Hyper+K");
  });

  it("ships no two commands claiming one chord", () => {
    // A shortcut doing something other than what its menu item says is the
    // exact drift this phase exists to remove, so it is a test rather than a
    // convention.
    expect(findKeybindingConflicts(commandDefinitions(context()))).toEqual([]);
  });

  it("reports a conflict when two commands do claim one chord", () => {
    const definitions = [
      { id: "a", title: "A", category: "T", keybinding: "Mod+K", availability: () => ({ kind: "available" as const }), run: vi.fn() },
      { id: "b", title: "B", category: "T", keybinding: "Mod+K", availability: () => ({ kind: "available" as const }), run: vi.fn() },
    ];
    expect(findKeybindingConflicts(definitions)).toEqual([
      { binding: "Mod+K", commandIds: ["a", "b"] },
    ]);
  });

  it("finds the command a keyboard event should run", () => {
    const ctx = context();
    const registry = createCommandRegistry(ctx);
    const event = { key: "k", ctrlKey: true, metaKey: false, shiftKey: false, altKey: false };
    expect(commandForEvent(registry, event, false)).toBe("app.palette");
  });

  it("returns nothing for an unbound key", () => {
    const ctx = context();
    const registry = createCommandRegistry(ctx);
    const event = { key: "j", ctrlKey: true, metaKey: false, shiftKey: false, altKey: false };
    expect(commandForEvent(registry, event, false)).toBeUndefined();
  });

  it("routes a shortcut through availability rather than round it", async () => {
    // The shortcut resolves an id; the registry decides whether it runs. A
    // keybinding that called `run` directly would work while its menu item was
    // greyed -- which is the bug in the opposite direction.
    const { actions, ran } = createActions();
    const ctx = context({ actions });
    const registry = createCommandRegistry(ctx);
    const event = { key: "w", ctrlKey: true, metaKey: false, shiftKey: false, altKey: false };
    const id = commandForEvent(registry, event, false);
    expect(id).toBe("task.close");
    expect(await registry.invoke(id!, ctx, "keybinding")).toEqual({
      kind: "unavailable",
      reason: "Open a task first.",
    });
    expect(ran).toEqual([]);
  });
});
