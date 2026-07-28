/**
 * Persisted workbench region sizes.
 *
 * FE-08 asks for two things this module owns: no panel renders below its usable
 * minimum, and a corrupt or old persisted layout falls back safely.
 *
 * "Safely" means the app opens. A layout is a convenience, and the failure mode
 * to avoid is a stored value -- from an older build, a hand-edited profile, a
 * half-written entry -- that leaves a panel at 3px or off-screen with no way to
 * drag it back. Every read is therefore validated and clamped rather than
 * trusted, and anything unreadable is discarded without comment.
 */

const STORAGE_KEY = "forge.workbench.layout";

/**
 * Bumped when the shape changes. A stored layout from a different version is
 * discarded rather than migrated: these are window sizes, and re-deriving them
 * from defaults costs the user one drag, where a bad migration costs a
 * confusing session.
 */
const SCHEMA_VERSION = 1;

export interface RegionBounds {
  /** Below this a region cannot show its content, so it is never rendered smaller. */
  min: number;
  max: number;
  initial: number;
}

export interface WorkbenchLayout {
  /** Region id -> width in CSS pixels. */
  readonly sizes: Readonly<Record<string, number>>;
}

const EMPTY: WorkbenchLayout = { sizes: {} };

/**
 * Clamps a size into a region's usable range.
 *
 * Exported because the resize handle needs exactly the same rule live as the
 * loader applies on read -- two implementations of a minimum is how a panel
 * ends up unusable only after a reload.
 */
export function clampToBounds(value: number, bounds: RegionBounds): number {
  if (!Number.isFinite(value)) return bounds.initial;
  return Math.min(bounds.max, Math.max(bounds.min, Math.round(value)));
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Reads the stored layout, or returns an empty one.
 *
 * Never throws. Storage can be unavailable entirely -- a privacy mode, a
 * disabled origin -- and a layout is not worth failing a boot over.
 */
export function loadLayout(storage: Storage | undefined = globalThis.localStorage): WorkbenchLayout {
  if (storage === undefined) return EMPTY;

  let raw: string | null;
  try {
    raw = storage.getItem(STORAGE_KEY);
  } catch {
    return EMPTY;
  }
  if (raw === null) return EMPTY;

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return EMPTY;
  }

  if (!isPlainRecord(parsed)) return EMPTY;
  if (parsed["version"] !== SCHEMA_VERSION) return EMPTY;
  if (!isPlainRecord(parsed["sizes"])) return EMPTY;

  // Individually validated: one bad entry discards that region, not the layout.
  // A user who has dragged four panels should not lose three of them because
  // the fourth was written during a crash.
  const sizes: Record<string, number> = {};
  for (const [id, value] of Object.entries(parsed["sizes"])) {
    if (typeof value === "number" && Number.isFinite(value) && value > 0) {
      sizes[id] = value;
    }
  }
  return { sizes };
}

/** Stores the layout. Never throws; a layout that cannot be saved is not an error. */
export function saveLayout(
  layout: WorkbenchLayout,
  storage: Storage | undefined = globalThis.localStorage,
): void {
  if (storage === undefined) return;
  try {
    storage.setItem(
      STORAGE_KEY,
      JSON.stringify({ version: SCHEMA_VERSION, sizes: layout.sizes }),
    );
  } catch {
    // Quota, privacy mode, or a disabled origin. The layout stays in memory.
  }
}

/**
 * The size a region should render at: stored if usable, otherwise its default.
 *
 * The clamp runs on read as well as on write, because bounds change between
 * builds. A width that was legal when it was stored can be below the minimum
 * after a redesign, and the stored value must not win that argument.
 */
export function resolveRegionSize(
  layout: WorkbenchLayout,
  regionId: string,
  bounds: RegionBounds,
): number {
  const stored = layout.sizes[regionId];
  if (stored === undefined) return bounds.initial;
  return clampToBounds(stored, bounds);
}

export const layoutStorageKey = STORAGE_KEY;
export const layoutSchemaVersion = SCHEMA_VERSION;
