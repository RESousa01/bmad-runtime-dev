# Support authority SQL identities and grants

The authority database uses Entra-only authentication with two separate
identities. Schema migration is an explicit deployment step; the runtime
identity can never alter schema.

## Migration identity (deployment step only)

Run once per deployment, as an Entra administrator of the database:

```sql
CREATE USER [sapphirus-support-migrator] FROM EXTERNAL PROVIDER;
ALTER ROLE db_ddladmin ADD MEMBER [sapphirus-support-migrator];
GRANT SELECT, INSERT ON dbo.desktop_schema_migrations
    TO [sapphirus-support-migrator];
```

Then apply migrations with `SqlMigrationRunner.ApplyAsync` (or by executing
`services/desktop-support-api/Sql/Migrations/*.sql` in name order and
recording each script's name and SHA-256 in `dbo.desktop_schema_migrations`).
The runner refuses to re-apply a migration whose recorded hash differs from
the embedded script.

## Runtime identity (user-assigned managed identity of the API)

Runtime DML only — no CREATE/ALTER/DROP, and no access to
`desktop_schema_migrations` beyond read:

```sql
CREATE USER [sapphirus-support-api] FROM EXTERNAL PROVIDER;
GRANT SELECT ON dbo.desktop_schema_migrations TO [sapphirus-support-api];
GRANT SELECT, INSERT, UPDATE ON dbo.desktop_device_registrations
    TO [sapphirus-support-api];
GRANT SELECT, INSERT ON dbo.desktop_entitlement_lease_audit
    TO [sapphirus-support-api];
GRANT SELECT, INSERT ON dbo.desktop_context_consent_consumptions
    TO [sapphirus-support-api];
GRANT SELECT, INSERT, UPDATE, DELETE ON dbo.desktop_request_idempotency
    TO [sapphirus-support-api];
GRANT SELECT, INSERT, UPDATE, DELETE ON dbo.desktop_model_call_idempotency
    TO [sapphirus-support-api];
GRANT SELECT, INSERT ON dbo.desktop_model_access_receipts
    TO [sapphirus-support-api];
GRANT INSERT ON dbo.desktop_security_audit TO [sapphirus-support-api];
```

Registrations and receipts are never deleted by the runtime; idempotency
claims are deleted only to release a failed claim. Revocation is an UPDATE
that flips state and increments the registration epoch — the transactional
active/epoch check in the API is the revocation authority.

## Content policy

No table contains prompt or output content, context labels, local paths,
source text, authorization tokens, provider credentials, or signatures beyond
required proof/audit material. The privacy canary integration tests
(`services/desktop-support-api.Tests/Sql/`) scan every text column after both
success and failure paths.
