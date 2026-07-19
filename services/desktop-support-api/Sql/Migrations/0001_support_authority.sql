-- D2-E support authority schema (Task 3).
-- Content policy: no table may contain prompt/output content, context labels,
-- local paths, source text, authorization tokens, provider credentials, or
-- signatures beyond required proof/audit material.

CREATE TABLE dbo.desktop_device_registrations (
    subject NVARCHAR(256) NOT NULL,
    registration_id NVARCHAR(96) NOT NULL,
    installation_public_key NVARCHAR(512) NOT NULL,
    installation_public_key_hash NVARCHAR(96) NOT NULL,
    client_release NVARCHAR(64) NOT NULL,
    platform NVARCHAR(32) NOT NULL,
    architecture NVARCHAR(32) NOT NULL,
    tenant_policy_version BIGINT NOT NULL,
    created_at DATETIMEOFFSET(3) NOT NULL,
    state NVARCHAR(16) NOT NULL,
    revoked_at DATETIMEOFFSET(3) NULL,
    epoch BIGINT NOT NULL,
    CONSTRAINT pk_desktop_device_registrations
        PRIMARY KEY (subject, registration_id),
    CONSTRAINT ck_desktop_device_registrations_state
        CHECK (state IN (N'active', N'revoked'))
);

CREATE TABLE dbo.desktop_entitlement_lease_audit (
    subject NVARCHAR(256) NOT NULL,
    lease_id NVARCHAR(96) NOT NULL,
    registration_id NVARCHAR(96) NOT NULL,
    lease_hash NVARCHAR(96) NOT NULL,
    issued_at DATETIMEOFFSET(3) NOT NULL,
    expires_at DATETIMEOFFSET(3) NOT NULL,
    recorded_at DATETIMEOFFSET(3) NOT NULL,
    CONSTRAINT pk_desktop_entitlement_lease_audit
        PRIMARY KEY (subject, lease_id)
);

CREATE TABLE dbo.desktop_context_consent_consumptions (
    subject_hash NVARCHAR(96) NOT NULL,
    registration_id NVARCHAR(96) NOT NULL,
    consumption_hash NVARCHAR(96) NOT NULL,
    decision_id NVARCHAR(96) NOT NULL,
    request_id NVARCHAR(96) NOT NULL,
    consumed_at DATETIMEOFFSET(3) NOT NULL,
    CONSTRAINT pk_desktop_context_consent_consumptions
        PRIMARY KEY (subject_hash, registration_id, consumption_hash)
);

CREATE TABLE dbo.desktop_request_idempotency (
    subject NVARCHAR(256) NOT NULL,
    idempotency_key NVARCHAR(128) NOT NULL,
    request_fingerprint NVARCHAR(128) NOT NULL,
    state NVARCHAR(16) NOT NULL,
    response_type NVARCHAR(256) NULL,
    response_json NVARCHAR(MAX) NULL,
    created_at DATETIMEOFFSET(3) NOT NULL,
    completed_at DATETIMEOFFSET(3) NULL,
    CONSTRAINT pk_desktop_request_idempotency
        PRIMARY KEY (subject, idempotency_key),
    CONSTRAINT ck_desktop_request_idempotency_state
        CHECK (state IN (N'started', N'completed'))
);

CREATE TABLE dbo.desktop_model_call_idempotency (
    subject NVARCHAR(256) NOT NULL,
    idempotency_key NVARCHAR(128) NOT NULL,
    request_fingerprint NVARCHAR(128) NOT NULL,
    state NVARCHAR(16) NOT NULL,
    receipt_id NVARCHAR(96) NULL,
    request_hash NVARCHAR(96) NULL,
    result_hash NVARCHAR(96) NULL,
    started_at DATETIMEOFFSET(3) NOT NULL,
    completed_at DATETIMEOFFSET(3) NULL,
    CONSTRAINT pk_desktop_model_call_idempotency
        PRIMARY KEY (subject, idempotency_key),
    CONSTRAINT ck_desktop_model_call_idempotency_state
        CHECK (state IN (N'started', N'completed'))
);

CREATE TABLE dbo.desktop_model_access_receipts (
    subject NVARCHAR(256) NOT NULL,
    receipt_id NVARCHAR(96) NOT NULL,
    request_id NVARCHAR(96) NOT NULL,
    request_hash NVARCHAR(96) NOT NULL,
    result_hash NVARCHAR(96) NOT NULL,
    receipt_json NVARCHAR(MAX) NOT NULL,
    recorded_at DATETIMEOFFSET(3) NOT NULL,
    CONSTRAINT pk_desktop_model_access_receipts
        PRIMARY KEY (subject, receipt_id)
);

CREATE TABLE dbo.desktop_security_audit (
    audit_id BIGINT IDENTITY(1, 1) NOT NULL,
    subject_hash NVARCHAR(96) NOT NULL,
    registration_id NVARCHAR(96) NULL,
    event_type NVARCHAR(64) NOT NULL,
    occurred_at DATETIMEOFFSET(3) NOT NULL,
    CONSTRAINT pk_desktop_security_audit PRIMARY KEY (audit_id)
);

CREATE INDEX ix_desktop_security_audit_subject
    ON dbo.desktop_security_audit (subject_hash, occurred_at);
