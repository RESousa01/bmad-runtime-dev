/// The renderer bundle's build identity, injected as `__FORGE_UI_BUILD__`.
///
/// `about.appVersion` is the crate version, so every build this repository has
/// ever produced reports "0.1.0" and a fault report naming it identifies
/// nothing. This is the half of a build identity the renderer can supply on its
/// own; a commit-pinned *host* identity needs a field on the About projection,
/// and `assertExactKeys` means that has to land on both sides at once.
///
/// Defined here rather than inline in `vite.config.ts` because `vitest.config.ts`
/// needs the identical value. Two configs each declaring their own copy is the
/// drift shape that keeps producing defects in this repository — a constant that
/// must agree across sites belongs at one site.
///
/// `GITHUB_SHA` is present in CI and absent locally, where "local" is the honest
/// answer rather than an invented identifier.
export function forgeUiBuildDefine(): Record<string, string> {
  const sha = process.env.GITHUB_SHA?.slice(0, 12) ?? "local";
  return {
    __FORGE_UI_BUILD__: JSON.stringify(`${new Date().toISOString()} ${sha}`),
  };
}
