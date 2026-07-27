// ESLint flat configuration for the first-party TypeScript/React source.
//
// Why this file exists
// --------------------
// Until it was added, `pnpm lint` in `apps/desktop-ui` was `tsc --noEmit` —
// byte-identical to `typecheck`. A green `lint` therefore asserted nothing that
// `typecheck` had not already asserted, across ~13k lines of React. The rule
// families that catch the defects a type checker structurally cannot see —
// stale hook dependencies, hooks called conditionally, refs read during render,
// interactive handlers on non-interactive elements — had never run.
//
// Why the Babel parser and not `typescript-eslint`
// ------------------------------------------------
// This repository pins TypeScript 7.0.2, the native (Go) compiler. Its npm
// package exports only `lib/version.cjs` from the package root — the classic
// JavaScript compiler API that `@typescript-eslint/typescript-estree` imports
// is not there, so that parser cannot run at all. Installing a second, older
// `typescript` alongside it is not an option either: `tools/check-boundaries.mjs`
// fails the build on any `typescript@` lock entry other than the pinned one.
//
// `@babel/eslint-parser` produces an ESTree AST for `.ts`/`.tsx` with no
// dependency on the TypeScript compiler, and both plugins used here are
// syntactic — neither needs type information. The cost is that type-aware rules
// are unavailable; `pnpm typecheck` remains the lane that covers types, and it
// is still run separately.
//
// Note on the parser options below: `@babel/eslint-parser` v8 takes a fast path
// when `babelrc` and `configFile` are both `false`, and that path reads *only*
// `babelOptions.parserOpts.plugins` — `presets: ["@babel/preset-typescript"]`
// is silently ignored and every file fails to parse at its first `import type`.
// The syntax plugins are therefore listed directly, and JSX is enabled only for
// `.tsx`, because the `jsx` plugin makes `<T>(x: T) => x` in a `.ts` file
// ambiguous with a JSX element.

import js from "@eslint/js";
import babelParser from "@babel/eslint-parser";
import jsxA11y from "eslint-plugin-jsx-a11y";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";

/// Rules that are correct for hand-written JavaScript but structurally wrong
/// for TypeScript read through a JavaScript-only scope analyser.
///
/// Neither is a judgement that the underlying problem does not matter — both
/// are already enforced, more precisely, by the compiler that `pnpm typecheck`
/// runs. Turning them off here removes ~2,060 false positives that would
/// otherwise have forced the whole gate to be advisory.
const typeScriptErasureOverrides = {
  // Type positions are ordinary identifier references to a JavaScript scope
  // analyser, so every `ReactNode`, `HostRuntime` and `ChatMessage` in a type
  // annotation reads as an undefined global (1,645 reports). TypeScript's own
  // resolver already errors on an unresolved type or value name.
  "no-undef": "off",
  // `import type { X }` and a parameter that exists only to satisfy a signature
  // both read as unused here (416 reports). `noUnusedLocals` and
  // `noUnusedParameters` are already on in `tsconfig.base.json`.
  "no-unused-vars": "off",
};

const typeScriptLanguageOptions = (parserPlugins) => ({
  parser: babelParser,
  parserOptions: {
    requireConfigFile: false,
    sourceType: "module",
    babelOptions: {
      babelrc: false,
      configFile: false,
      parserOpts: { plugins: parserPlugins },
    },
  },
  globals: { ...globals.browser },
});

/// Two narrowings of `jsx-a11y`, each aimed at a pattern the rule cannot see.
/// Neither turns a rule off: both leave every other trigger reporting.
const pointerHandlers = ["onClick", "onMouseDown", "onMouseUp", "onKeyPress", "onKeyUp"];

const jsxA11yOverrides = {
  // `onKeyDown` dropped from the handler list for these two rules only.
  //
  // Their purpose is to catch an element that has been made into a control
  // without the semantics of one. A fake control always has a pointer handler,
  // and every pointer handler is still listed — so the rules still fire on the
  // thing they exist for. What they were also firing on is a container that
  // delegates one key: Escape to dismiss the attach menu, the agent popover and
  // the rename form; Tab to keep focus inside the workspace dialog and the two
  // shell panels. Six sites, none of them a control, all of them correct.
  //
  // The alternative was six `eslint-disable` comments repeating one reason,
  // which is a worse record of the decision than one place stating it.
  "jsx-a11y/no-static-element-interactions": ["error", { handlers: pointerHandlers }],
  "jsx-a11y/no-noninteractive-element-interactions": [
    "error",
    { handlers: pointerHandlers },
  ],
  // A scrollable `<pre>` needs `tabindex="0"` or a keyboard user cannot scroll
  // it at all (WCAG 2.1.1). The rule's premise — that a tabindex on a
  // non-interactive element is a mistake — is inverted for a scroll container,
  // so `pre` is allowlisted and nothing else is. Both current uses (the raw
  // message JSON and the read-only file preview) are exactly that.
  "jsx-a11y/no-noninteractive-tabindex": [
    "error",
    { tags: ["pre"], roles: ["tabpanel"], allowExpressionValues: true },
  ],
};

const reactRules = {
  ...js.configs.recommended.rules,
  ...typeScriptErasureOverrides,
  ...reactHooks.configs.recommended.rules,
  // Stated explicitly rather than inherited at `warn`, because a stale
  // dependency array is the defect class this configuration exists to catch:
  // the projection poller closed over `markReadOnlyRecovery` and
  // `markHostUnavailable` without declaring them, and the recovery-expiry
  // effect declared two fields of `recoveryReview` while its body used the
  // whole object. Both are read-only-recovery paths — the app's worst failure
  // mode is exactly where the unchecked dependencies were.
  "react-hooks/exhaustive-deps": "error",
};

export default [
  {
    ignores: [
      "**/dist/**",
      "**/coverage/**",
      "**/node_modules/**",
      "**/target/**",
      "artifacts/**",
      "output/**",
    ],
  },
  {
    files: ["**/*.ts"],
    languageOptions: typeScriptLanguageOptions(["typescript"]),
    plugins: { "react-hooks": reactHooks },
    rules: reactRules,
  },
  {
    files: ["**/*.tsx"],
    languageOptions: typeScriptLanguageOptions(["typescript", "jsx"]),
    plugins: { "react-hooks": reactHooks, "jsx-a11y": jsxA11y },
    rules: {
      ...reactRules,
      ...jsxA11y.flatConfigs.recommended.rules,
      ...jsxA11yOverrides,
    },
  },
  {
    // Plain JavaScript in the source lane — currently the visual-contract test.
    // It gets the core rules at full strength, including the two disabled
    // above, because the TypeScript erasure problem does not apply here.
    files: ["**/*.{js,mjs,cjs}"],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
      globals: { ...globals.node },
    },
    rules: js.configs.recommended.rules,
  },
];
