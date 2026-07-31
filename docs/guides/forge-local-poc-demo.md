# Forge local PoC demo

This runbook demonstrates the bounded local Forge proof of concept. It does not
claim production readiness.

## Preconditions

- Install the repository's locked dependencies.
- Install the Rust and Tauri prerequisites.
- Have a local project folder available.
- Have either an OpenAI API key, or an Azure OpenAI API key, deployment name,
  and Azure OpenAI v1 endpoint.
- Never paste a Foundry project endpoint into the local provider field.

## Launch

```bash
pnpm poc:check
pnpm poc:dev
```

## Recommended demo sequence

1. Open a local workspace.
2. Open Settings → Providers.
3. Select OpenAI or Azure OpenAI.
4. For Azure, enter the deployment name and
   `https://<resource>.openai.azure.com/openai/v1/`.
5. Connect. Explain that the key stays in native memory for the app session.
6. Create a task.
7. Attach `README.md` and one small source or planning file.
8. Open Skills & agents and start **Create PRD**.
9. Click **Add attached files (2)**.
10. Select `Create` and `Interactive`.
11. Prepare the reviewed context, inspect the exact paths and content, approve,
    and submit.
12. Show the rendered document artifact.
13. Return to chat. The attachments remain armed until a chat send. Ask:

    ```text
    Inspect the attached project context and the workspace when needed. Explain what Forge currently does, what is deliberately read-only, and the three highest-value next steps. Use read, list, glob, or grep when useful.
    ```

14. Show any read-only tool requests and results, then the final response.
15. State the PoC boundary: no writes, shell, Builder activation, or managed
    Foundry support plane.

## Important attachment note

A successful chat send intentionally clears attachments so they are not
silently resent. Run the PRD demonstration before chat, or reattach the files
before starting the PRD run.

## Failure fallback

- Wrong Azure endpoint: use the Azure OpenAI v1 endpoint, not the Foundry
  project endpoint.
- Credential rejected: reconnect with a valid key.
- Model or deployment unavailable: verify the deployment identifier.
- Output rejected: retry once with the same small context; do not disable
  response validation.
- No key available: demonstrate only the local shell and workspace flow, and
  label it as a host/UI fallback rather than the model-backed PoC.
