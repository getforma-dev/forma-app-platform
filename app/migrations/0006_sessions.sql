-- ADR-006 and ADR-007 D7.3/D7.5/D7.9/D7.11 (APP-009, plan Task 7): the three
-- session relations of the sign-in contract. Pre-login flows, pre-access
-- sessions and workspace sessions are organization-leading (D7.5) and each
-- states ENABLE/FORCE RLS and the tenant_scope policy here (D7.3). Every
-- cookie value is stored only as a SHA-256 digest; the PKCE verifier only as
-- ciphertext under a process-memory key; the fAuth refresh token never: a row
-- holds only a reference to the process-memory vault entry (D7.11). No nonce
-- column: fAuth issues no id_token (ADR-006). Grants are D7.3's genui_workspace
-- row: column SELECT and INSERT, UPDATE(last_seen_at, revoked_at), DELETE.
-- One further column read (marked below) lets a fresh verified sign-in find its
-- own live pending admission (ADR-006 Flow 2); it is an open question for an
-- ADR-007 amendment, recorded in evidence/APP-009. Executed only by the
-- checksummed, transactional migration runner (D7.12: numbered in landing order).
SET LOCAL ROLE genui_owner;

-- D7.12: each migration 000k ALTERs both version CHECKs to BETWEEN 1 AND k.
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 6);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 6);

-- ADR-006 pre-login flow: 10 minutes absolute, single use, consumed or
-- discarded at the callback. A signup flow is bound to its pre-signup
-- reservation (APP-074's invitation entry creates it).
CREATE TABLE genui.prelogin_flows (
 organization_id uuid NOT NULL REFERENCES genui.organizations, flow_id uuid NOT NULL,
 cookie_digest genui.digest NOT NULL, state_digest genui.digest NOT NULL,
 kind text NOT NULL CHECK (kind IN ('signin','signup')),
 reservation_id uuid, signup_handoff_id uuid,
 code_verifier_ciphertext bytea NOT NULL CHECK (octet_length(code_verifier_ciphertext) BETWEEN 1 AND 256),
 client_address_class text NOT NULL CHECK (length(client_address_class) BETWEEN 1 AND 64),
 created_at timestamptz NOT NULL, expires_at timestamptz NOT NULL,
 PRIMARY KEY (organization_id,flow_id), UNIQUE (organization_id,cookie_digest),
 FOREIGN KEY (organization_id,reservation_id) REFERENCES genui.code_reservations,
 CHECK ((kind = 'signin' AND reservation_id IS NULL AND signup_handoff_id IS NULL)
  OR (kind = 'signup' AND reservation_id IS NOT NULL)),
 CHECK (expires_at > created_at AND expires_at <= created_at + interval '10 minutes')
);
-- ADR-006 pre-access session: a verified identity that is not (yet) a member.
-- 10 minutes idle (judged on last_seen_at), 30 minutes absolute.
CREATE TABLE genui.pre_access_sessions (
 organization_id uuid NOT NULL REFERENCES genui.organizations, session_id uuid NOT NULL,
 cookie_digest genui.digest NOT NULL,
 issuer text NOT NULL, provider_tenant_id text NOT NULL,
 provider_subject text NOT NULL CHECK (length(provider_subject) BETWEEN 1 AND 256),
 email_verified boolean NOT NULL,
 flow_kind text NOT NULL CHECK (flow_kind IN ('signin','signup')),
 pending_admission_id uuid, pending_expires_at timestamptz,
 refresh_token_ref uuid NOT NULL,
 created_at timestamptz NOT NULL, last_seen_at timestamptz NOT NULL, expires_at timestamptz NOT NULL,
 revoked_at timestamptz,
 PRIMARY KEY (organization_id,session_id), UNIQUE (organization_id,cookie_digest),
 UNIQUE (organization_id,refresh_token_ref),
 FOREIGN KEY (organization_id,issuer,provider_tenant_id) REFERENCES genui.provider_tenant_bindings,
 FOREIGN KEY (organization_id,pending_admission_id) REFERENCES genui.code_reservations,
 CHECK ((pending_admission_id IS NULL) = (pending_expires_at IS NULL)),
 CHECK (last_seen_at >= created_at),
 CHECK (expires_at > created_at AND expires_at <= created_at + interval '30 minutes')
);
-- ADR-006 workspace session: 1 hour idle (judged on last_seen_at), 12 hours
-- absolute (design §6.1, M24), bound to the member, the epoch at minting, the
-- entitlement and the registry identity.
CREATE TABLE genui.sessions (
 organization_id uuid NOT NULL, session_id uuid NOT NULL, cookie_digest genui.digest NOT NULL,
 principal_id uuid NOT NULL, membership_epoch_at_mint genui.safe_counter NOT NULL,
 entitlement_id uuid NOT NULL,
 registry_sha256 text NOT NULL CHECK (registry_sha256 ~ '^[0-9a-f]{64}$'),
 refresh_token_ref uuid NOT NULL,
 created_at timestamptz NOT NULL, last_seen_at timestamptz NOT NULL, absolute_expires_at timestamptz NOT NULL,
 revoked_at timestamptz,
 PRIMARY KEY (organization_id,session_id), UNIQUE (organization_id,cookie_digest),
 UNIQUE (organization_id,refresh_token_ref),
 FOREIGN KEY (organization_id,principal_id) REFERENCES genui.memberships,
 FOREIGN KEY (organization_id,principal_id,entitlement_id) REFERENCES genui.entitlements,
 CHECK (last_seen_at >= created_at),
 CHECK (absolute_expires_at > created_at AND absolute_expires_at <= created_at + interval '12 hours')
);
CREATE INDEX sessions_principal ON genui.sessions (organization_id,principal_id);

-- D7.3: every new genui relation states ENABLE/FORCE RLS and tenant_scope here.
DO $$
DECLARE t text;
BEGIN
 FOREACH t IN ARRAY ARRAY['prelogin_flows','pre_access_sessions','sessions'] LOOP
  EXECUTE format('ALTER TABLE genui.%I ENABLE ROW LEVEL SECURITY',t);
  EXECUTE format('ALTER TABLE genui.%I FORCE ROW LEVEL SECURITY',t);
  EXECUTE format('CREATE POLICY tenant_scope ON genui.%I USING (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid) WITH CHECK (organization_id = nullif(current_setting(''genui.organization_id'',true),'''')::uuid)',t);
 END LOOP;
END $$;

-- D7.3 genui_workspace (APP-009): column reads and inserts, the two
-- column-limited updates, and DELETE (sign-out, single use, expiry sweep).
GRANT SELECT(organization_id,flow_id,cookie_digest,state_digest,kind,reservation_id,signup_handoff_id,
 code_verifier_ciphertext,client_address_class,created_at,expires_at),
 INSERT(organization_id,flow_id,cookie_digest,state_digest,kind,reservation_id,signup_handoff_id,
 code_verifier_ciphertext,client_address_class,created_at,expires_at),
 DELETE ON genui.prelogin_flows TO genui_workspace;
GRANT SELECT(organization_id,session_id,cookie_digest,issuer,provider_tenant_id,provider_subject,email_verified,flow_kind,
 pending_admission_id,pending_expires_at,refresh_token_ref,created_at,last_seen_at,expires_at,revoked_at),
 INSERT(organization_id,session_id,cookie_digest,issuer,provider_tenant_id,provider_subject,email_verified,flow_kind,
 pending_admission_id,pending_expires_at,refresh_token_ref,created_at,last_seen_at,expires_at),
 UPDATE(last_seen_at,revoked_at), DELETE ON genui.pre_access_sessions TO genui_workspace;
GRANT SELECT(organization_id,session_id,cookie_digest,principal_id,membership_epoch_at_mint,entitlement_id,registry_sha256,
 refresh_token_ref,created_at,last_seen_at,absolute_expires_at,revoked_at),
 INSERT(organization_id,session_id,cookie_digest,principal_id,membership_epoch_at_mint,entitlement_id,registry_sha256,
 refresh_token_ref,created_at,last_seen_at,absolute_expires_at),
 UPDATE(last_seen_at,revoked_at), DELETE ON genui.sessions TO genui_workspace;
-- Open question (evidence/APP-009): ADR-006 Flow 2 activates on a later,
-- ordinary sign-in whose exchange reports email_verified true, which must find
-- that identity's live pending admission to call admit_redemption. D7.3 grants
-- no read of code_reservations; this column read (no code_id, no context
-- digest) is the narrow means chosen here, pending an ADR-007 amendment.
GRANT SELECT(organization_id,reservation_id,kind,issuer,provider_tenant_id,provider_subject,state,expires_at)
 ON genui.code_reservations TO genui_workspace;
RESET ROLE;
