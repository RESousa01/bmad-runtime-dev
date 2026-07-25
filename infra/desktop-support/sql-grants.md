# Support authority SQL identities and grants

The authority database uses Entra-only authentication with two separate
identities. Schema migration is an explicit deployment step; the runtime
identity can never alter schema.

## Migration identity (deployment step only)

Run once for a clean database, as an Entra administrator of the database.
The administrator creates the migration ledger before granting object
permissions so the bootstrap works when no schema exists yet:

```sql
CREATE USER [sapphirus-support-migrator] FROM EXTERNAL PROVIDER;
ALTER ROLE db_ddladmin ADD MEMBER [sapphirus-support-migrator];
IF OBJECT_ID(N'dbo.desktop_schema_migrations', N'U') IS NULL
BEGIN
    CREATE TABLE dbo.desktop_schema_migrations (
        migration_name NVARCHAR(128) NOT NULL,
        script_sha256 NVARCHAR(96) NOT NULL,
        applied_at DATETIMEOFFSET(3) NOT NULL,
        CONSTRAINT pk_desktop_schema_migrations
            PRIMARY KEY (migration_name)
    );
END;
GRANT SELECT, INSERT ON dbo.desktop_schema_migrations
    TO [sapphirus-support-migrator];
```

Then run the published support API image once with the exact `migrate-sql`
argument under the migration identity, as documented in the deployment guide.
That command is the only supported migration path. It composes Production
configuration, invokes `SqlMigrationRunner.ApplyAsync`, and exits nonzero on
any failure without starting the web server. Never execute an embedded script
or write a migration-ledger row manually. The runner refuses to re-apply a
migration whose recorded hash differs from the embedded script.

## Runtime identity (user-assigned managed identity of the API)

Run this block as an Entra administrator only **after every migration has
completed**. The referenced application tables do not exist on a clean
database before that point. Runtime DML only — no CREATE/ALTER/DROP, and no
access to `desktop_schema_migrations` beyond read:

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
