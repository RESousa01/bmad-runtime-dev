# Support plane and broker baseline — CL-02

**Recorded:** 2026-07-28
**Backlog item:** CL-02, phase 0, depends on CL-01
**Environment:** Windows 11, .NET SDK 10.0.302 (matches `global.json`, `rollForward: disable`), Azure CLI with the Bicep extension available

## Result, by the plan's own verification commands

| Check | State | Evidence |
|---|---|---|
| `dotnet test services/desktop-support-api.Tests/… -c Release` | **passed** | 181 tests, 181 succeeded, 0 failed |
| `dotnet publish helpers/windows-auth-broker/… -c Release -r win-x64` | **passed** | publishes clean; requires Windows, which this host is |
| `az bicep build --file infra/desktop-support/main.bicep` | **passed** | one warning, see below |
| `dotnet restore Sapphirus.slnx --locked-mode` (CL-01) | **passed** | all five projects against reviewed locks |
| `dotnet build Sapphirus.slnx -c Release` (CL-01) | **passed** | 0 warnings, 0 errors |

## Acceptance

**1. The baseline distinguishes checks not run due to missing Azure or Windows
from failed checks.**

Every check above ran on this host, so none are in the unavailable state today.
The distinction still has to be stated, because it is exactly what a Linux CI
runner or a machine without the Azure CLI will hit:

| State | Meaning | How it appears |
|---|---|---|
| **passed** | ran here, succeeded | exit 0 |
| **failed** | ran here, did not succeed | non-zero exit, output recorded |
| **unavailable** | could not run on this host | named with the missing prerequisite, never counted as a pass |

Which checks can go unavailable, and on what:

- the broker publish needs a Windows host — `-r win-x64` cross-publishes, but
  the broker is a Windows authentication component and only Windows can prove it;
- `az bicep build` needs the Azure CLI with the Bicep extension;
- anything touching a deployed environment needs credentials this repository
  deliberately does not hold. Those are CL-24 and CL-25, not this item.

An unavailable check is **not** a passing check. The failure mode this criterion
exists to prevent is a green run that silently skipped the Windows and Azure
halves — and there is a live example of the same shape elsewhere in the gate:
`verify:source` stops at its Node-pin assertion and reports one failure for
fifteen unrun steps. That is recorded in `runtime-baseline.md` under RT-02.

**2. Generated infrastructure JSON is reproducible.**

Verified. `infra/desktop-support/main.json` is tracked; regenerating it with
`az bicep build --file infra/desktop-support/main.bicep` leaves it **byte
identical** — SHA-256 unchanged, empty diff.

## The one warning, and why it stands

`az bicep build` emits a single warning: the `Asserts` experimental feature is
enabled. That is **BIC-007** in
`docs/reviews/sapphirus-desktop-support-bicep-issues.md`, where it is recorded
as intentional. It is the only remaining build warning for this template.

## What this baseline does not claim

That the support plane works when deployed. Everything above is build-time and
local-test evidence. Audit finding F-14 makes the same point: the API has
strict JSON handling, Entra authentication, rate limiting, device registration,
consent, policy, idempotency, model brokering, SQL and signing, and **no
deployed end-to-end qualification** — so desktop-to-Azure trust claims are
unproven until a real environment is exercised.

Two things block that, and neither is code:

- the Bicep register carries seven placeholder values only the tenant owner can
  supply (`CFG-001` through `CFG-007`: API audience, SQL administrator object
  ID, model version, and four canonical profile hashes);
- `NET-001` — the Container Apps environment sets `publicNetworkAccess: 'Enabled'`
  with external ingress while private endpoints exist alongside it. Audit finding
  F-16 reaches the same conclusion independently. The perimeter is a decision
  before it is a change.
