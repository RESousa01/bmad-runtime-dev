/**
 * One registry every command entry point goes through.
 *
 * FE-09's two acceptance criteria are the whole design:
 *
 *   1. the same command produces the same availability and result from every
 *      entry point;
 *   2. unavailability always has a user-readable reason.
 *
 * The first is why availability is a property of the *command*, not of the
 * button. When a palette, a menu item, a shortcut and a toolbar button each
 * decide separately whether an action can run, they drift, and the drift shows
 * up as a shortcut that works while its menu item is greyed -- or worse, the
 * other way round.
 *
 * The second is enforced by the type. `unavailable` carries a required
 * `reason`, so there is no way to mark a command unavailable without saying
 * why. A greyed control with no explanation is the failure this prevents: the
 * user can see that something is impossible and has no idea what to change.
 */

/** Whether a command can run right now, and if not, why not. */
export type CommandAvailability =
  | { readonly kind: "available" }
  | {
    readonly kind: "unavailable";
    /**
     * Shown to the user verbatim. A sentence naming what would make the
     * command available, not an error code and not "not allowed".
     */
    readonly reason: string;
  };

export const available: CommandAvailability = { kind: "available" };

export function unavailable(reason: string): CommandAvailability {
  return { kind: "unavailable", reason };
}

/** Where a command was invoked from. Recorded so drift between them is visible. */
export type CommandSource = "palette" | "menu" | "keybinding" | "toolbar" | "programmatic";

export interface CommandDefinition<Context> {
  readonly id: string;
  /** Shown in the palette and in menus. Imperative: "Apply changes". */
  readonly title: string;
  /** Groups the command in the palette. */
  readonly category: string;
  /**
   * Extra words the palette should match on. The title is always searched;
   * this is for the words a user might reach for instead.
   */
  readonly keywords?: readonly string[];
  /**
   * Chord in the platform-neutral form: "Mod+Enter", "Mod+Shift+P".
   * `Mod` resolves to Cmd on macOS and Ctrl elsewhere, so a binding is
   * declared once rather than per platform.
   */
  readonly keybinding?: string;
  /** Whether the command can run, given the current context. */
  readonly availability: (context: Context) => CommandAvailability;
  readonly run: (context: Context, source: CommandSource) => void | Promise<void>;
}

export interface ResolvedCommand<Context> {
  readonly definition: CommandDefinition<Context>;
  readonly availability: CommandAvailability;
}

export class CommandRegistry<Context> {
  readonly #commands = new Map<string, CommandDefinition<Context>>();

  constructor(definitions: readonly CommandDefinition<Context>[] = []) {
    for (const definition of definitions) this.register(definition);
  }

  register(definition: CommandDefinition<Context>): void {
    if (this.#commands.has(definition.id)) {
      // Two definitions of one id is the drift this registry exists to stop,
      // and it is always a mistake rather than a merge.
      throw new Error(`Command "${definition.id}" is already registered.`);
    }
    this.#commands.set(definition.id, definition);
  }

  get(id: string): CommandDefinition<Context> | undefined {
    return this.#commands.get(id);
  }

  list(): readonly CommandDefinition<Context>[] {
    return [...this.#commands.values()];
  }

  /**
   * Resolves a command against the current context.
   *
   * Every entry point calls this. A caller that decides availability for
   * itself is the bug -- so the palette, the menu and the keybinding handler
   * below all route through it, and so should any new surface.
   */
  resolve(id: string, context: Context): ResolvedCommand<Context> | undefined {
    const definition = this.#commands.get(id);
    if (definition === undefined) return undefined;
    return { definition, availability: definition.availability(context) };
  }

  /** Every command with its current availability, for the palette. */
  resolveAll(context: Context): readonly ResolvedCommand<Context>[] {
    return this.list().map((definition) => ({
      definition,
      availability: definition.availability(context),
    }));
  }

  /**
   * Runs a command if it is available.
   *
   * Returns what happened rather than throwing, because "unavailable" is an
   * ordinary outcome that the caller usually wants to show rather than handle.
   * The availability check is here and not at the call site, so a surface
   * cannot invoke something it should have greyed.
   */
  async invoke(
    id: string,
    context: Context,
    source: CommandSource,
  ): Promise<
    | { readonly kind: "ran" }
    | { readonly kind: "unknown" }
    | { readonly kind: "unavailable"; readonly reason: string }
  > {
    const resolved = this.resolve(id, context);
    if (resolved === undefined) return { kind: "unknown" };
    if (resolved.availability.kind === "unavailable") {
      return { kind: "unavailable", reason: resolved.availability.reason };
    }
    await resolved.definition.run(context, source);
    return { kind: "ran" };
  }
}
