/**
 * The keybinding half of the command registry.
 *
 * Bindings are declared once, platform-neutrally, as `Mod+Enter` or
 * `Mod+Shift+P`. `Mod` resolves to Cmd on macOS and Ctrl everywhere else, so a
 * command does not carry two spellings of the same shortcut and cannot drift
 * between them.
 *
 * This is also where the audit's note about "macOS-only shortcut glyphs"
 * (section 7.2) is answered: `formatKeybinding` renders the label for the
 * platform the user is actually on, rather than shipping the glyph the mockup
 * happened to be drawn on.
 */

import type { CommandDefinition, CommandRegistry } from "./commandRegistry";

export interface Chord {
  readonly key: string;
  readonly mod: boolean;
  readonly shift: boolean;
  readonly alt: boolean;
}

/** True on macOS, where `Mod` means Cmd and the glyphs differ. */
export function isApplePlatform(
  platform: string = globalThis.navigator?.platform ?? "",
): boolean {
  return /mac|iphone|ipad|ipod/iu.test(platform);
}

/** Parses `Mod+Shift+P`. Returns null for anything malformed. */
export function parseChord(binding: string): Chord | null {
  const parts = binding.split("+").map((part) => part.trim()).filter(Boolean);
  if (parts.length === 0) return null;

  const key = parts.at(-1);
  if (key === undefined || key.length === 0) return null;

  let mod = false;
  let shift = false;
  let alt = false;
  for (const part of parts.slice(0, -1)) {
    switch (part.toLowerCase()) {
      case "mod": mod = true; break;
      case "shift": shift = true; break;
      case "alt": alt = true; break;
      default: return null;
    }
  }
  return { key: key.toLowerCase(), mod, shift, alt };
}

/** Whether a keyboard event is this chord. */
export function matchesChord(
  event: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "shiftKey" | "altKey">,
  chord: Chord,
  apple: boolean = isApplePlatform(),
): boolean {
  const modPressed = apple ? event.metaKey : event.ctrlKey;
  // The other modifier must be *absent*, or Ctrl+Enter on macOS would fire a
  // Cmd+Enter binding and vice versa.
  const otherMod = apple ? event.ctrlKey : event.metaKey;
  return (
    event.key.toLowerCase() === chord.key
    && modPressed === chord.mod
    && !otherMod
    && event.shiftKey === chord.shift
    && event.altKey === chord.alt
  );
}

/** The label to show, in the conventions of the platform the user is on. */
export function formatKeybinding(binding: string, apple: boolean = isApplePlatform()): string {
  const chord = parseChord(binding);
  if (chord === null) return binding;
  const parts: string[] = [];
  if (chord.mod) parts.push(apple ? "⌘" : "Ctrl");
  if (chord.shift) parts.push(apple ? "⇧" : "Shift");
  if (chord.alt) parts.push(apple ? "⌥" : "Alt");
  const key = chord.key === "enter"
    ? (apple ? "⏎" : "Enter")
    : chord.key.length === 1 ? chord.key.toUpperCase() : chord.key;
  parts.push(key);
  return apple ? parts.join("") : parts.join("+");
}

/**
 * Two commands claiming one chord.
 *
 * Reported rather than resolved. Silently letting the first or the last win is
 * how a shortcut starts doing something other than what its menu item says.
 */
export interface KeybindingConflict {
  readonly binding: string;
  readonly commandIds: readonly string[];
}

export function findKeybindingConflicts<Context>(
  definitions: readonly CommandDefinition<Context>[],
): readonly KeybindingConflict[] {
  const byChord = new Map<string, string[]>();
  for (const definition of definitions) {
    if (definition.keybinding === undefined) continue;
    const chord = parseChord(definition.keybinding);
    if (chord === null) continue;
    const key = `${chord.mod ? "M" : ""}${chord.shift ? "S" : ""}${chord.alt ? "A" : ""}:${chord.key}`;
    const existing = byChord.get(key);
    if (existing) existing.push(definition.id);
    else byChord.set(key, [definition.id]);
  }
  const conflicts: KeybindingConflict[] = [];
  for (const [, commandIds] of byChord) {
    if (commandIds.length < 2) continue;
    const first = definitions.find((d) => d.id === commandIds[0]);
    conflicts.push({ binding: first?.keybinding ?? "", commandIds });
  }
  return conflicts;
}

/**
 * Finds the command a keyboard event should invoke.
 *
 * It resolves the id only; invoking still goes through the registry, so a
 * shortcut cannot run something the palette would have shown as unavailable.
 */
export function commandForEvent<Context>(
  registry: CommandRegistry<Context>,
  event: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "shiftKey" | "altKey">,
  apple: boolean = isApplePlatform(),
): string | undefined {
  for (const definition of registry.list()) {
    if (definition.keybinding === undefined) continue;
    const chord = parseChord(definition.keybinding);
    if (chord !== null && matchesChord(event, chord, apple)) return definition.id;
  }
  return undefined;
}

