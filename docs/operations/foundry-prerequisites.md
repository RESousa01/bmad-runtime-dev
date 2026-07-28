# Microsoft Foundry prerequisite check

Date: 2026-07-28

Run this check before any Forge task chooses a Foundry deployment or changes
the production model path. A dependent task may proceed only when
`artifacts/foundry/prerequisites.json` says `"ready": true`.

## Required operator inputs

Set these values in the invoking process. Do not commit them to the repository.

| Environment variable | Required value |
|---|---|
| `FOUNDRY_PROJECT_ENDPOINT` | Complete project endpoint: `https://<account>.services.ai.azure.com/api/projects/<project>` or the documented `_project` form. |
| `FOUNDRY_MODEL_IDENTITY_CLIENT_ID` | Client ID of the user-assigned identity used by the model broker. |
| `FOUNDRY_CONTAINER_APP_RESOURCE_ID` | Complete ARM resource ID of the Container App that must have the identity attached. |
| `FOUNDRY_PROJECT_RESOURCE_ID` | Exact Foundry account or project ARM scope on which authorization is expected. Its account and project must agree with the data-plane endpoint. When the endpoint uses `_project`, supply the account scope; a project scope cannot be bound to that alias and is rejected. |
| `FOUNDRY_PROFILE_DEPLOYMENTS` | Comma-separated deployment names explicitly approved for the first Forge profiles. These are deployment names, not catalog model names. |

The Azure CLI must also be authenticated in the intended tenant and able to
read the Foundry and Container App resources. The checker requests only a
short-lived token for `https://ai.azure.com/.default`; it never writes the
token to disk.

## Run

```powershell
pwsh -File tools/foundry-smoke/check-prerequisites.ps1 `
  -ProjectEndpoint "$env:FOUNDRY_PROJECT_ENDPOINT" `
  -ManagedIdentityClientId "$env:FOUNDRY_MODEL_IDENTITY_CLIENT_ID"
```

Optional command parameters correspond to the remaining environment variables
and support an explicit output path. The default report is
`artifacts/foundry/prerequisites.json`.

Exit code `0` means the report is ready. Exit code `2` means the check completed
but one or more prerequisites are blocked. Some command hosts surface any
nonzero native child result as `1`; the report is the authoritative
classification.

## What the checker proves

1. The endpoint is HTTPS on the default port, has no user info, query, or
   fragment, uses the exact `.services.ai.azure.com` suffix, and has exactly
   `/api/projects/<project>` or `/api/projects/_project`.
2. The current Azure identity can acquire the documented Entra token without
   persisting it.
3. `GET <project-endpoint>/deployments?api-version=v1&deploymentType=ModelDeployment`
   returns a bounded JSON inventory. Pagination must stay on the same origin
   and project deployment path, retain `api-version=v1`, avoid cycles, and
   finish within 32 pages.
4. The report retains only deployment name, publisher, model name/version,
   bounded SKU fields, deployment type, and capability keys. Connection names,
   capability values, headers, tokens, and raw response fields are omitted.
5. Every intended deployment name is explicitly supplied and present. The
   checker does not assume that a deployment name equals a model name.
6. The user-assigned identity is attached to the exact Container App.
7. The identity has the Foundry User role at the intended scope or an
   inherited parent scope. The stable role definition ID
   `53ca6127-db72-4b80-b1b0-d745d6d5456d` is used because Microsoft is rolling
   out the rename from Azure AI User to Foundry User.

The deployment API shape and Entra scope are defined in the
[Microsoft Foundry REST reference](https://learn.microsoft.com/en-us/rest/api/microsoft-foundry/aiproject).
The role ID and verification command are defined in Microsoft's
[Foundry authentication and authorization guidance](https://learn.microsoft.com/en-us/azure/foundry/concepts/authentication-authorization-foundry).

## Report safety

The report contains booleans, stable blocker codes, the validated project
endpoint, redacted deployment metadata, and SHA-256 labels for Azure identity
and resource identifiers. It must never contain:

- bearer tokens or authorization headers;
- API keys or client secrets;
- managed-identity client, principal, or resource IDs in clear text;
- Container App or role-assignment response bodies;
- connection names or capability values;
- raw Azure CLI or HTTP error text.

## Current local result

The 2026-07-28 check is blocked because the Azure CLI is not authenticated and
none of the five operator inputs above are present. No real deployment,
identity attachment, or role proof is claimed. FND-010 and every other task
that depends on FND-002 must remain blocked until a fresh report says
`"ready": true`.
