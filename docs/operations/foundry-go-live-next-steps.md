# Make Forge Foundry Work: Simple Next Steps

This guide covers only the actions still needed to prove that Microsoft
Foundry is working with Forge. The application code is already mostly in
place. The remaining work needs a real Foundry project, a real backend API
key, a staging deployment, and live verification.

> Important: never paste the Foundry API key into this guide, source control,
> a parameter file, a Codex message, logs, screenshots, or an Azure deployment
> command. Store it in a protected deployment secret.

## What success looks like

Foundry is ready only when all of these are true:

- [ ] The prerequisite report says `"ready": true`.
- [ ] Forge shows at least one approved model profile.
- [ ] A normal chat request succeeds.
- [ ] A structured BMAD request succeeds.
- [ ] The returned receipt verifies.
- [ ] A wrong or revoked key fails safely.
- [ ] A new key can be introduced without interrupting the old revision.
- [ ] Rollback to the previous revision works.
- [ ] The final scorecard says `"complete": true`.

## Step 1: Collect the required Azure information

In Microsoft Foundry and the Azure portal, collect:

- The complete project endpoint:

  ```text
  https://<account>.services.ai.azure.com/api/projects/<project>
  ```

- A Foundry API key intended only for the Forge backend.
- A short non-secret key version, such as `v1` or `2026-07-rotation-1`.
- The route or deployment ID for each model Forge should use.
- The model publisher, model name, and model version for each route.
- The Foundry project Azure resource ID.
- The Azure subscription and resource group containing the Forge Container
  App.
- A small approved model-call budget for testing.

Do not continue with only the account hostname. The endpoint must include
`/api/projects/<project>`.

## Step 2: Prepare the real model profiles

Use
`infra/desktop-support/model-profiles.example.json` as the shape, but create
the real file outside the repository, for example:

```text
C:\secure\forge\model-profiles.json
```

For each enabled profile, replace the example values with:

- the real `deploymentName`;
- the verified publisher, model name, and model version;
- the capabilities and limits you actually tested;
- a real qualification evidence hash and timestamp.

Keep the file out of Git. Do not put the API key or project endpoint in it.

## Step 3: Prove the endpoint, key, profile, and route

Open PowerShell in the repository and set the non-secret values:

```powershell
$env:FOUNDRY_PROJECT_ENDPOINT = 'https://<account>.services.ai.azure.com/api/projects/<project>'
$env:FOUNDRY_API_KEY_VERSION = 'v1'
$env:FOUNDRY_INVENTORY_MODE = 'configured_and_probed'
```

Enter the key without placing it in PowerShell history:

```powershell
$env:FOUNDRY_API_KEY = Read-Host 'Paste the Foundry API key' -MaskInput
```

Run the prerequisite check:

```powershell
pwsh -NoProfile -File tools/foundry-smoke/check-prerequisites.ps1 `
  -ProjectEndpoint "$env:FOUNDRY_PROJECT_ENDPOINT" `
  -ApiKeyEnvironmentVariable FOUNDRY_API_KEY `
  -Profiles 'C:\secure\forge\model-profiles.json' `
  -InventoryMode configured_and_probed `
  -BudgetMicrounits 5000
```

Open `artifacts/foundry/prerequisites.json`.

Continue only when:

- `ready` is `true`;
- `blockers` is empty;
- `apiKeyPresent` is `true`;
- `profilesValidated` is `true`;
- `responsesCompatibleRouteProved` is `true`;
- `foundryEntraTokenAcquired` is `false`;
- `selectedInventoryMode` is the mode you deliberately chose.

If the report is blocked, fix the named blocker and rerun the same command.
Do not change inventory modes merely to make the check pass.

When finished, remove the key from the current PowerShell process:

```powershell
Remove-Item Env:FOUNDRY_API_KEY
```

## Step 4: Clear the local Forge verification gates

Run:

```powershell
pwsh -NoProfile -File tools/foundry-smoke/local-verify.ps1
```

Open `artifacts/foundry/qualification/local-matrix.json` and require:

```text
ready = true
failed = 0
unavailableMandatory = 0
```

The current checkout still has two known local blockers:

- the native generated-contract output root must be restored;
- Windows must allow the Rust build helper to launch.

These are code/tooling tasks for Codex to repair or rerun in a suitable
Windows environment. Do not treat partial local results as deployment proof.

## Step 5: Deploy an inactive staging revision

Use the protected deployment pipeline to:

1. Pass the API key through the secure `foundryApiKey` parameter.
2. Store it as the Container Apps secret named `foundry-api-key`.
3. Inject it only through the `Sapphirus__FoundryApiKey` `secretRef`.
4. Set the complete project endpoint.
5. Set the qualified inventory mode.
6. Set the prerequisite evidence hash.
7. Deploy a new inactive Container App revision.

Before applying the deployment, run Bicep build, validation, and authenticated
what-if. Review the what-if and confirm:

- no existing Foundry project or model deployment is deleted;
- no API key appears in outputs or plain environment values;
- no Foundry managed identity or Foundry role assignment is created;
- the container image uses the intended immutable digest;
- the previous Container App revision remains available for rollback.

Do not send production traffic to the new revision yet.

## Step 6: Test from inside the new revision

From the inactive revision, run:

```powershell
pwsh -NoProfile -File tools/foundry-smoke/network-smoke.ps1 `
  -ProjectEndpoint "$env:FOUNDRY_PROJECT_ENDPOINT"
```

The running container should already receive
`Sapphirus__FoundryApiKey` from its secret reference. Do not pass the key on
the command line.

Require the network report to prove:

- DNS resolves the correct Foundry host;
- TLS/SNI succeeds;
- an API-key-authenticated Foundry request succeeds;
- the report contains no key, endpoint, route name, response body, or raw
  address;
- a Foundry failure affects model access only, not bootstrap, policy, or
  device routes.

## Step 7: Run the real Forge journeys

Against the inactive staging revision, verify:

1. Sign in with the configured Forge desktop/session mechanism.
2. Open the model catalog.
3. Confirm only approved profiles are visible.
4. Select a profile and restart Forge; confirm the stable profile selection
   survives.
5. Send a normal chat message and receive a complete stream.
6. Stop one stream and confirm cancellation is clean.
7. Run a structured BMAD request.
8. Verify its signed receipt on the desktop.
9. Remove or disable a route and confirm Forge fails closed while preserving
   the draft.
10. Try a wrong key and confirm the error is
    `foundry_authentication_failed` without exposing the upstream response.
11. Check logs, traces, SQL, problems, receipts, and deployment outputs for
    the test prompt, path canary, API-key-shaped canary, route name, endpoint,
    or headers. None may appear in plaintext.

Keep screenshots and machine-readable results as staging evidence, but never
capture the API key.

## Step 8: Prove key rotation and rollback

Key rotation:

1. Create a replacement Foundry key.
2. Deploy another inactive revision using the replacement secret.
3. Repeat the prerequisite, network, chat, BMAD, receipt, and privacy checks.
4. Shift a small amount of traffic to the new revision.
5. Confirm it remains healthy.
6. Shift the remaining traffic.
7. Revoke the old key.
8. Confirm the old revision now fails model access safely.

Rollback:

1. Keep the previous key valid during the agreed rollback window.
2. Shift traffic back to the previous revision.
3. Restore the previous compatible model-profile registry version.
4. Confirm catalog, chat, BMAD, and receipt verification recover.

Do not revoke the previous key before the new revision and rollback path have
both been tested.

## Step 9: Ask Codex for the final check

Once the evidence above exists, ask Codex to run:

```powershell
node tools/foundry-smoke/verify-final.mjs --strict
```

Open `artifacts/foundry/final-scorecard.json`.

Foundry is fully qualified only when:

```text
complete = true
blockers = []
```

If either condition is false, use the blocker list as the remaining work
queue. Do not describe Foundry as production-ready before this final result.

## Quick troubleshooting

- `project_endpoint_missing`
  Supply the complete `/api/projects/<project>` URL.

- `api_key_missing` or `api_key_rejected`
  Confirm the key is present in the current process or Container Apps secret
  reference and has not been revoked.

- `profiles_missing`
  Pass the path to the real model-profile document.

- `callable_profile_route_missing`
  Correct the profile route ID or deploy/enable the intended route.

- `responses_canary_budget_unavailable`
  Approve a small test budget and rerun the configured route probe.

- Local matrix reports contract or Rust failures
  Return the report to Codex; these are repository/tooling issues, not a
  reason to change the Foundry key or inventory mode.
