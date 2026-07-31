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

test("desktop support CI smokes the exact built image and the Production boot contract", () => {
  const source = workflows.find(({ name }) => name === "desktop-support.yml")?.source;
  assert.ok(source, "desktop-support.yml must exist");
  const triggerSource = source.split(/^permissions:\s*$/mu)[0];

  for (const path of [
    ".dockerignore",
    ".node-version",
    "apps/desktop-ui/**",
    "infra/local/**",
    "packages/**",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "tsconfig.base.json",
    "tools/forge-local.test.mjs",
    "tools/support-cloud-contracts.test.mjs",
    "tools/check-workflows.test.mjs",
    "tools/support-smoke/**",
    "package.json",
  ]) {
    const occurrences = triggerSource.match(
      new RegExp(path.replaceAll(/[.*+?^${}()|[\]\\]/gu, "\\$&"), "gu"),
    ) ?? [];
    assert.equal(occurrences.length, 2, `${path} must trigger on pull_request and push`);
  }

  assert.match(source, /--tag desktop-support-api:ci/u);
  assert.match(
    source,
    /node --test [^\n]*tools\/support-cloud-contracts\.test\.mjs/u,
  );
  assert.match(
    source,
    /docker run --rm[\s\S]*?desktop-support-api:ci migrate-sql/u,
  );
  assert.match(source, /migration_exit[\s\S]*?-ne 1/u);
  assert.match(
    source,
    /ASPNETCORE_ENVIRONMENT=Production[\s\S]*?Sapphirus__ManagedIdentityClientId=44444444-4444-4444-4444-444444444444[\s\S]*?Sapphirus__SqlMigrationIdentityClientId=44444444-4444-4444-4444-444444444444[\s\S]*?desktop-support-api:ci migrate-sql/u,
  );
  assert.match(source, /production_identity_exit[\s\S]*?-ne 1/u);
  assert.match(
    source,
    /--network none[\s\S]*?ASPNETCORE_ENVIRONMENT=Production/u,
  );
  assert.match(source, /DESKTOP_SUPPORT_IMAGE=desktop-support-api:ci/u);
  assert.match(source, /--no-build --wait desktop-support-api/u);
  assert.match(source, /tools\/support-smoke\/local-smoke\.ps1/u);
  assert.match(source, /--profile production-configuration-contract/u);
  assert.match(source, /--no-build --wait production-configuration-contract/u);
  assert.match(source, /docker image inspect desktop-support-api:ci/u);
  assert.match(source, /docker inspect [^\n]*\{\{\.Image\}\}/u);
  assert.match(source, /trap cleanup EXIT/u);
  assert.match(source, /down --volumes --remove-orphans/u);
});

test("desktop support CI builds and smokes the exact frontend image", () => {
  const source = workflows.find(({ name }) => name === "desktop-support.yml")?.source;
  assert.ok(source, "desktop-support.yml must exist");

  assert.match(
    source,
    /docker build --file infra\/local\/frontend\.Dockerfile --tag forge-frontend:ci \./u,
  );
  assert.match(source, /FORGE_FRONTEND_IMAGE=forge-frontend:ci/u);
  assert.match(source, /FORGE_FRONTEND_PORT=1420/u);
  assert.match(source, /frontend_compose_file=infra\/local\/compose\.yaml/u);
  assert.match(source, /frontend_project=forge-frontend-ci/u);
  assert.match(
    source,
    /docker compose -p "\$frontend_project" -f "\$frontend_compose_file" up\s+--detach --no-build --wait frontend/u,
  );
  assert.match(source, /docker image inspect forge-frontend:ci/u);
  assert.match(
    source,
    /docker compose -p "\$frontend_project" -f "\$frontend_compose_file" ps -q frontend/u,
  );
  assert.match(
    source,
    /assert_exact_image "\$frontend_expected_image" "\$frontend_container_id" frontend/u,
  );
  assert.match(source, /State\.Health\.Status/u);
  assert.match(
    source,
    /frontend_http_status="\$\(curl --silent --show-error --output \/dev\/null --write-out '%\{http_code\}' http:\/\/127\.0\.0\.1:1420\/\)"/u,
  );
  assert.match(
    source,
    /if \[ "\$frontend_http_status" != "200" \]; then/u,
  );
  assert.match(
    source,
    /cleanup\(\)[\s\S]*?\$frontend_compose_file[\s\S]*?down --volumes --remove-orphans[\s\S]*?\$support_compose_file[\s\S]*?down --volumes --remove-orphans/u,
  );
  assert.match(source, /trap cleanup EXIT/u);
});

test("automated review excludes only temporary imported vendor snapshots", () => {
  const codeqlWorkflow = workflows.find(({ name }) => name === "codeql.yml")?.source;
  assert.ok(codeqlWorkflow, "codeql.yml must exist");
  assert.match(codeqlWorkflow, /config-file:\s+\.\/\.github\/codeql-config\.yml/u);
  assert.match(codeqlWorkflow, /language:\s+csharp[\s\S]*?build-mode:\s+none/u);
  assert.doesNotMatch(codeqlWorkflow, /Build reviewed C# targets/u);
  assert.doesNotMatch(codeqlConfig, /^\s+- bmad-runtime-lib\/\*\*\s*$/mu);
  assert.doesNotMatch(codeRabbitConfig, /^\s+- "!bmad-runtime-lib\/\*\*"\s*$/mu);
  for (const path of [
    "_source_review/BMAD-METHOD-main",
    "_source_review/bmad-builder-main",
    "_source_review/openclaw-main",
    "_source_review/hermes-agent-main",
    "_source_review/odysseus-dev",
    "_full",
  ]) {
    const escaped = path.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
    assert.match(codeqlConfig, new RegExp(`^\\s+- bmad-runtime-lib/${escaped}/\\*\\*\\s*$`, "mu"));
    assert.match(codeRabbitConfig, new RegExp(`^\\s+- "!bmad-runtime-lib/${escaped}/\\*\\*"\\s*$`, "mu"));
  }
  assert.match(
    readFileSync(new URL("knowledge/verify-archive-manifest.mjs", import.meta.url), "utf8"),
    /archive-manifest/u,
    "archive integrity verification must remain enabled",
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
