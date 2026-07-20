import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import test from "node:test";

const workflowDirectory = new URL("../.github/workflows/", import.meta.url);
const workflows = readdirSync(workflowDirectory)
  .filter((name) => name.endsWith(".yml") || name.endsWith(".yaml"))
  .map((name) => ({
    name,
    source: readFileSync(new URL(name, workflowDirectory), "utf8"),
  }));
const codeqlConfig = readFileSync(new URL("../.github/codeql-config.yml", import.meta.url), "utf8");
const codeRabbitConfig = readFileSync(new URL("../.coderabbit.yaml", import.meta.url), "utf8");

function workflowStepBlocks(source) {
  const lines = source.split(/\r?\n/u);
  const starts = lines
    .map((line, index) => (/^      - (?:name|uses|run):/u.test(line) ? index : -1))
    .filter((index) => index >= 0);
  return starts.map((start, index) =>
    lines.slice(start, starts[index + 1] ?? lines.length).join("\n"));
}

test("workflows use bounded, immutable, current CI primitives", () => {
  for (const { name, source } of workflows) {
    assert.match(
      source,
      /^concurrency:[\s\S]*?^  cancel-in-progress:\s*true\s*$/mu,
      `${name} must cancel superseded runs`,
    );
    assert.doesNotMatch(
      source,
      /(?:ubuntu|windows|macos)-latest/u,
      `${name} must pin its runner image`,
    );
    assert.doesNotMatch(source, /cargo install cargo-deny/u, `${name} must not compile cargo-deny on every run`);
    assert.doesNotMatch(source, /sql inspection wired via runbook/u, `${name} must not substitute prose for a gate`);

    const jobsSource = source.split(/^jobs:\s*$/mu)[1] ?? "";
    const jobCount = [...jobsSource.matchAll(/^  [a-zA-Z][a-zA-Z0-9_-]*:\s*$/gmu)].length;
    const timeoutCount = [...jobsSource.matchAll(/^    timeout-minutes:\s*\d+\s*$/gmu)].length;
    assert.equal(timeoutCount, jobCount, `${name} must bound every job with timeout-minutes`);

    for (const action of source.matchAll(/uses:\s*([^\s#]+)(?:\s*#.*)?$/gmu)) {
      const reference = action[1];
      if (reference.startsWith("./") || reference.startsWith("docker://")) continue;
      assert.match(reference, /^[^@]+@[0-9a-f]{40}$/u, `${name} must pin ${reference} by full commit SHA`);
    }
  }
});

test("checkout never leaves a writable repository credential behind", () => {
  for (const { name, source } of workflows) {
    const checkoutSteps = workflowStepBlocks(source)
      .filter((step) => /^      - uses:\s*actions\/checkout@/mu.test(step));
    for (const step of checkoutSteps) {
      assert.match(
        step,
        /^          persist-credentials:\s*false\s*$/mu,
        `${name} must disable checkout credential persistence`,
      );
    }
  }
});

test("desktop support changes always trigger their own workflow and run a real SQL gate", () => {
  const source = workflows.find(({ name }) => name === "desktop-support.yml")?.source;
  assert.ok(source, "desktop-support.yml must exist");
  assert.match(source, /push:[\s\S]*?\.github\/workflows\/desktop-support\.yml/u);
  const privacyGateStep = workflowStepBlocks(source)
    .find((step) => /^      - name:\s*Privacy gate SQL inspection\s*$/mu.test(step));
  assert.ok(privacyGateStep, "desktop-support.yml must contain the privacy SQL run step");
  assert.match(privacyGateStep, /^        run:\s*\|\s*$/mu);
  assert.match(
    privacyGateStep,
    /sqlcmd\s+-G\s+-b[\s\S]*?-i\s+tools\/support-smoke\/privacy-sql-inspect\.sql/u,
  );
  assert.match(
    privacyGateStep,
    /if\s*\(\$LASTEXITCODE\s+-ne\s+0\)\s*\{\s*exit\s+\$LASTEXITCODE\s*\}/u,
  );
});

test("automated review excludes the read-only upstream source vault", () => {
  const codeqlWorkflow = workflows.find(({ name }) => name === "codeql.yml")?.source;
  assert.ok(codeqlWorkflow, "codeql.yml must exist");
  assert.match(codeqlWorkflow, /config-file:\s+\.\/\.github\/codeql-config\.yml/u);
  assert.match(codeqlWorkflow, /language:\s+csharp[\s\S]*?build-mode:\s+none/u);
  assert.doesNotMatch(codeqlWorkflow, /Build reviewed C# targets/u);
  assert.match(codeqlConfig, /^\s+- bmad-runtime-lib\/\*\*\s*$/mu);
  assert.match(codeRabbitConfig, /^\s+- "!bmad-runtime-lib\/\*\*"\s*$/mu);
  assert.match(
    readFileSync(new URL("verify-reference-vault.mjs", import.meta.url), "utf8"),
    /bmad-runtime-lib/u,
    "reference-vault integrity verification must remain enabled",
  );
});

test("generator qualification does not duplicate the desktop all-feature Rust gate", () => {
  const contractsWorkflow = workflows.find(({ name }) => name === "contracts.yml")?.source;
  const desktopWorkflow = workflows.find(({ name }) => name === "desktop.yml")?.source;
  assert.ok(contractsWorkflow, "contracts.yml must exist");
  assert.ok(desktopWorkflow, "desktop.yml must exist");
  assert.doesNotMatch(contractsWorkflow, /cargo clippy --workspace --all-targets --all-features/u);
  assert.doesNotMatch(contractsWorkflow, /cargo test --workspace --all-features/u);
  assert.match(desktopWorkflow, /cargo clippy --workspace --all-targets --all-features --locked/u);
  assert.match(desktopWorkflow, /cargo test --workspace --all-features --locked/u);
});
