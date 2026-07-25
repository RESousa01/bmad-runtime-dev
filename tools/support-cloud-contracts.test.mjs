import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const read = (path) =>
  readFile(new URL(`../${path}`, import.meta.url), "utf8");

test("clean SQL bootstrap creates the migration ledger before grants and defers runtime grants", async () => {
  const grants = await read("infra/desktop-support/sql-grants.md");
  const guide = await read(
    "docs/guides/azure-support-plane-deployment-guide.md",
  );

  const ledgerCreate = grants.indexOf(
    "CREATE TABLE dbo.desktop_schema_migrations",
  );
  const migratorGrant = grants.indexOf(
    "GRANT SELECT, INSERT ON dbo.desktop_schema_migrations",
  );
  assert.ok(ledgerCreate >= 0, "the admin bootstrap must create the ledger");
  assert.ok(
    ledgerCreate < migratorGrant,
    "the migration ledger must exist before its object grant",
  );

  const migrationIdentity = guide.indexOf(
    "### 4.1 Bootstrap the migration identity and ledger",
  );
  const migrations = guide.indexOf(
    "### 5.2 Apply migrations under the migration identity",
  );
  const runtimeIdentity = guide.indexOf(
    "### 5.3 Create the runtime identity and apply DML grants",
  );
  assert.ok(
    migrationIdentity >= 0
      && migrationIdentity < migrations
      && migrations < runtimeIdentity,
    "the runbook must order ledger bootstrap, migrations, then runtime grants",
  );
  assert.match(
    guide.slice(migrationIdentity, migrations),
    /migration identity block only/iu,
  );
  assert.match(
    guide.slice(runtimeIdentity),
    /runtime identity\s+block only/iu,
  );
});

test("the support image exposes one fail-closed production migration command", async () => {
  const program = await read("services/desktop-support-api/Program.cs");
  const dockerfile = await read("services/desktop-support-api/Dockerfile");
  const appsettings = JSON.parse(
    await read("services/desktop-support-api/appsettings.json"),
  );
  const guide = await read(
    "docs/guides/azure-support-plane-deployment-guide.md",
  );
  const grants = await read("infra/desktop-support/sql-grants.md");
  const bicep = await read("infra/desktop-support/main.bicep");
  const compiled = JSON.parse(await read("infra/desktop-support/main.json"));

  assert.match(program, /SqlMigrationCommand\.Select\(args\)/u);
  assert.match(
    program,
    /return await SqlMigrationCommand\.ExecuteAsync\(/u,
    "migration mode must return before the web server can start",
  );
  assert.match(
    dockerfile,
    /ENTRYPOINT \["dotnet", "Sapphirus\.DesktopSupportApi\.dll"\]/u,
    "the image entrypoint must append the migration command as an argument",
  );
  assert.equal(
    Object.hasOwn(
      appsettings.Sapphirus,
      "SqlMigrationIdentityClientId",
    ),
    false,
    "command-only migration identity must not break normal Production binding",
  );

  const migrationStage = guide.slice(
    guide.indexOf("### 5.2 Apply migrations under the migration identity"),
    guide.indexOf("### 5.3 Create the runtime identity and apply DML grants"),
  );
  assert.match(migrationStage, /\bmigrate-sql\b/u);
  assert.match(migrationStage, /nonzero/iu);
  assert.match(migrationStage, /never starts the web server/iu);
  assert.match(migrationStage, /migration identity/iu);
  assert.doesNotMatch(migrationStage, /apply the script manually/iu);
  assert.doesNotMatch(migrationStage, /insert .+ row yourself/iu);
  assert.doesNotMatch(grants, /executing.+Migrations\/\*\.sql/isu);
  assert.match(bicep, /name: '\$\{namePrefix\}-sqlmigrate-id'/u);
  assert.match(
    guide,
    /<namePrefix>-sqlmigrate-id/u,
    "the runbook must use the migration identity name provisioned by Bicep",
  );

  assert.doesNotMatch(
    guide,
    /show-manifests[\s\S]{0,160}\[0\]\.digest/u,
    "migration image selection must never use the latest manifest",
  );
  assert.doesNotMatch(
    guide.slice(
      guide.indexOf("### 5.1 Consume the trusted release image by digest"),
      guide.indexOf("### 5.2 Apply migrations under the migration identity"),
    ),
    /desktop-support-api:build/u,
    "the published tag must be unique to the reviewed source revision",
  );
  const publicationStage = guide.slice(
    guide.indexOf("### 5.1 Consume the trusted release image by digest"),
    guide.indexOf("### 5.2 Apply migrations under the migration identity"),
  );
  assert.doesNotMatch(
    publicationStage,
    /^\s*docker (?:build|push)\b/mu,
    "the operator runbook must not build or push a replacement image",
  );
  assert.match(publicationStage, /organization.+release lane/iu);
  assert.match(publicationStage, /SBOM/iu);
  assert.match(publicationStage, /sign(?:ed|ature|ing)/iu);
  assert.match(publicationStage, /attestation/iu);
  assert.match(publicationStage, /blocked until/iu);
  assert.match(publicationStage, /\$publishedDigest/u);
  assert.match(publicationStage, /\$resolvedDigest/u);
  assert.match(
    publicationStage,
    /\$resolvedDigest -cne \$publishedDigest/u,
    "the ACR tag must resolve to the exact digest emitted by the trusted lane",
  );
  assert.match(guide, /\$imageTag = "sha-\$sourceRevision"/u);
  assert.match(
    guide,
    /az acr repository show[\s\S]{0,240}--image "desktop-support-api:\$imageTag"[\s\S]{0,120}--query digest/u,
  );
  assert.match(guide, /\^sha256:\[0-9a-f\]\{64\}\$/u);

  for (const output of [
    "apiManagedIdentityClientId",
    "apiManagedIdentityName",
    "imagePullIdentityResourceId",
    "sqlMigrationIdentityResourceId",
    "sqlMigrationIdentityName",
  ]) {
    assert.match(
      bicep,
      new RegExp(`output ${output} string`, "u"),
      `Bicep must output ${output} for deterministic job/grant wiring`,
    );
    assert.match(
      guide,
      new RegExp(`\\b${output}\\b`, "u"),
      `the runbook must consume or record ${output}`,
    );
    assert.ok(
      compiled.outputs?.[output],
      `the compiled ARM template must include ${output}`,
    );
  }
});
