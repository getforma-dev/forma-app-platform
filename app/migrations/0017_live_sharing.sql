-- Named live sharing uses account authority independent of tenant membership.
-- Existing org tables and TrustedRequest/member fences are unchanged. Account
-- relations have their own account_scope RLS; no guest receives org grants.
CREATE SCHEMA genui_identity AUTHORIZATION genui_owner;
SET LOCAL ROLE genui_owner;
ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK(version BETWEEN 1 AND 17);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK(version BETWEEN 1 AND 17);
REVOKE ALL ON SCHEMA genui_identity FROM PUBLIC;
GRANT USAGE ON SCHEMA genui_identity TO genui_workspace,genui_data;

CREATE TABLE genui_identity.accounts (
 account_id uuid PRIMARY KEY,
 issuer text NOT NULL CHECK(length(issuer) BETWEEN 1 AND 2048),
 provider_tenant_id text NOT NULL CHECK(length(provider_tenant_id) BETWEEN 1 AND 256),
 provider_subject text NOT NULL CHECK(length(provider_subject) BETWEEN 1 AND 256),
 verified_email text NOT NULL CHECK(length(verified_email) BETWEEN 3 AND 320),
 account_epoch bigint NOT NULL DEFAULT 1 CHECK(account_epoch BETWEEN 1 AND 9007199254740991),
 status text NOT NULL DEFAULT 'active' CHECK(status IN ('active','revoked')),
 created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(),
 audit_sequence bigint NOT NULL DEFAULT 0 CHECK(audit_sequence BETWEEN 0 AND 9007199254740991),
 audit_hash genui.digest NOT NULL DEFAULT 'sha256:0000000000000000000000000000000000000000000000000000000000000000',
 UNIQUE(issuer,provider_tenant_id,provider_subject)
);
CREATE TABLE genui_identity.shared_sessions (
 account_id uuid NOT NULL REFERENCES genui_identity.accounts,
 session_id uuid NOT NULL,
 cookie_digest genui.digest NOT NULL,
 refresh_token_ref uuid NOT NULL,
 account_epoch bigint NOT NULL CHECK(account_epoch BETWEEN 1 AND 9007199254740991),
 created_at timestamptz NOT NULL DEFAULT now(),last_seen_at timestamptz NOT NULL DEFAULT now(),expires_at timestamptz NOT NULL DEFAULT now()+interval '12 hours',revoked_at timestamptz,
 PRIMARY KEY(account_id,session_id), UNIQUE(account_id,cookie_digest),
 CHECK(expires_at>created_at AND expires_at<=created_at+interval '12 hours')
);
CREATE TABLE genui_identity.shared_auth_flows (
 flow_id uuid PRIMARY KEY,
 owner_organization_id uuid NOT NULL REFERENCES genui.organizations,
 share_id uuid NOT NULL,
 provider_organization_id uuid NOT NULL REFERENCES genui.organizations,
 cookie_digest genui.digest NOT NULL UNIQUE,
 state_digest genui.digest NOT NULL,
 sealed_verifier bytea NOT NULL CHECK(octet_length(sealed_verifier) BETWEEN 1 AND 4096),
 created_at timestamptz NOT NULL DEFAULT now(),
 expires_at timestamptz NOT NULL DEFAULT now()+interval '10 minutes',consumed_at timestamptz,
 CHECK(expires_at>created_at AND expires_at<=created_at+interval '10 minutes')
);
CREATE TABLE genui.live_work_shares (
 organization_id uuid NOT NULL,share_id uuid NOT NULL,
 app_id uuid NOT NULL,work_id uuid NOT NULL,
 sponsor_id uuid NOT NULL,sponsor_epoch bigint NOT NULL CHECK(sponsor_epoch BETWEEN 1 AND 9007199254740991),
 recipient_email text NOT NULL CHECK(length(recipient_email) BETWEEN 3 AND 320),
 accepted_account_id uuid REFERENCES genui_identity.accounts,accepted_at timestamptz,
 publication_id uuid NOT NULL,definition_digest genui.digest NOT NULL,
 work_version bigint NOT NULL CHECK(work_version BETWEEN 1 AND 9007199254740991),
 view_ids text[] NOT NULL CHECK(cardinality(view_ids)<=64 AND array_position(view_ids,NULL) IS NULL),
 collection_ids text[] NOT NULL CHECK(cardinality(collection_ids)<=8 AND array_position(collection_ids,NULL) IS NULL),
 title text NOT NULL CHECK(length(title) BETWEEN 1 AND 240),
 allow_add_to_canvas boolean NOT NULL,
 version bigint NOT NULL CHECK(version BETWEEN 1 AND 9007199254740991),
 idempotency_key text NOT NULL CHECK(length(idempotency_key) BETWEEN 16 AND 128),request_digest genui.digest NOT NULL,
 created_at timestamptz NOT NULL,expires_at timestamptz NOT NULL,revoked_at timestamptz,
 PRIMARY KEY(organization_id,share_id),
 UNIQUE(organization_id,app_id,work_id,sponsor_id,idempotency_key),
 FOREIGN KEY(organization_id,app_id,work_id) REFERENCES genui.saved_work,
 FOREIGN KEY(organization_id,sponsor_id) REFERENCES genui.memberships,
 FOREIGN KEY(organization_id,app_id,publication_id) REFERENCES genui.publications,
 CHECK((accepted_account_id IS NULL)=(accepted_at IS NULL)),
 CHECK(expires_at>created_at AND expires_at<=created_at+interval '30 days'+interval '1 second')
);
ALTER TABLE genui_identity.shared_auth_flows ADD FOREIGN KEY(owner_organization_id,share_id) REFERENCES genui.live_work_shares(organization_id,share_id);
CREATE TABLE genui_identity.linked_apps (
 account_id uuid NOT NULL REFERENCES genui_identity.accounts,
 organization_id uuid NOT NULL,share_id uuid NOT NULL,
 created_at timestamptz NOT NULL,removed_at timestamptz,
 grant_version bigint NOT NULL CHECK(grant_version BETWEEN 1 AND 9007199254740991),
 placement_generation bigint NOT NULL DEFAULT 1 CHECK(placement_generation BETWEEN 1 AND 9007199254740991),
 PRIMARY KEY(account_id,organization_id,share_id),
 FOREIGN KEY(organization_id,share_id) REFERENCES genui.live_work_shares
);
CREATE TABLE genui_identity.share_events (
 account_id uuid NOT NULL REFERENCES genui_identity.accounts,event_id uuid NOT NULL,
 sequence bigint NOT NULL CHECK(sequence BETWEEN 1 AND 9007199254740991),
 organization_id uuid NOT NULL,share_id uuid NOT NULL,
 kind text NOT NULL CHECK(kind IN ('share.accepted','app.linked','app.unlinked')),
 share_version bigint NOT NULL CHECK(share_version BETWEEN 0 AND 9007199254740991),
 canonical bytea NOT NULL CHECK(octet_length(canonical) BETWEEN 1 AND 4096),
 previous_hash genui.digest NOT NULL,hash genui.digest NOT NULL,occurred_at timestamptz NOT NULL,
 PRIMARY KEY(account_id,event_id),UNIQUE(account_id,sequence),
 FOREIGN KEY(organization_id,share_id) REFERENCES genui.live_work_shares
);

ALTER TABLE genui.live_work_shares ENABLE ROW LEVEL SECURITY;
ALTER TABLE genui.live_work_shares FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_scope ON genui.live_work_shares
 USING(organization_id=nullif(current_setting('genui.organization_id',true),'')::uuid)
 WITH CHECK(organization_id=nullif(current_setting('genui.organization_id',true),'')::uuid);
DO $$ DECLARE name text; BEGIN
 FOREACH name IN ARRAY ARRAY['accounts','shared_sessions','linked_apps','share_events'] LOOP
  EXECUTE format('ALTER TABLE genui_identity.%I ENABLE ROW LEVEL SECURITY',name);
  EXECUTE format('ALTER TABLE genui_identity.%I FORCE ROW LEVEL SECURITY',name);
  EXECUTE format('CREATE POLICY account_scope ON genui_identity.%I USING(account_id=nullif(current_setting(''genui.account_id'',true),'''')::uuid) WITH CHECK(account_id=nullif(current_setting(''genui.account_id'',true),'''')::uuid)',name);
 END LOOP;
END $$;
ALTER TABLE genui_identity.shared_auth_flows ENABLE ROW LEVEL SECURITY;
ALTER TABLE genui_identity.shared_auth_flows FORCE ROW LEVEL SECURITY;
CREATE POLICY account_scope ON genui_identity.shared_auth_flows
 USING(flow_id=nullif(current_setting('genui.account_id',true),'')::uuid)
 WITH CHECK(flow_id=nullif(current_setting('genui.account_id',true),'')::uuid);

GRANT SELECT(account_id,issuer,provider_tenant_id,provider_subject,verified_email,account_epoch,status,created_at,updated_at,audit_sequence,audit_hash),
 INSERT(account_id,issuer,provider_tenant_id,provider_subject,verified_email,account_epoch,status,created_at,updated_at),
 UPDATE(verified_email,account_epoch,status,updated_at,audit_sequence,audit_hash) ON genui_identity.accounts TO genui_workspace;
GRANT SELECT(account_id,account_epoch,status) ON genui_identity.accounts TO genui_data;
GRANT SELECT(account_id,session_id,cookie_digest,refresh_token_ref,account_epoch,created_at,last_seen_at,expires_at,revoked_at),
 INSERT(account_id,session_id,cookie_digest,refresh_token_ref,account_epoch,created_at,last_seen_at,expires_at,revoked_at),
 UPDATE(last_seen_at,revoked_at) ON genui_identity.shared_sessions TO genui_workspace;
GRANT DELETE ON genui_identity.shared_sessions TO genui_workspace;
GRANT SELECT(account_id,session_id,account_epoch,last_seen_at,expires_at,revoked_at) ON genui_identity.shared_sessions TO genui_data;
GRANT SELECT(flow_id,owner_organization_id,share_id,provider_organization_id,cookie_digest,state_digest,sealed_verifier,created_at,expires_at,consumed_at),
 INSERT(flow_id,owner_organization_id,share_id,provider_organization_id,cookie_digest,state_digest,sealed_verifier,created_at,expires_at),
 UPDATE(consumed_at) ON genui_identity.shared_auth_flows TO genui_workspace;
GRANT DELETE ON genui_identity.shared_auth_flows TO genui_workspace;
GRANT SELECT(organization_id,share_id,app_id,work_id,sponsor_id,sponsor_epoch,recipient_email,accepted_account_id,accepted_at,publication_id,definition_digest,work_version,view_ids,collection_ids,title,allow_add_to_canvas,version,idempotency_key,request_digest,created_at,expires_at,revoked_at),
 INSERT(organization_id,share_id,app_id,work_id,sponsor_id,sponsor_epoch,recipient_email,publication_id,definition_digest,work_version,view_ids,collection_ids,title,allow_add_to_canvas,version,idempotency_key,request_digest,created_at,expires_at),
 UPDATE(accepted_account_id,accepted_at,allow_add_to_canvas,version,revoked_at) ON genui.live_work_shares TO genui_workspace;
GRANT SELECT(organization_id,share_id,app_id,work_id,sponsor_id,sponsor_epoch,accepted_account_id,publication_id,definition_digest,work_version,view_ids,collection_ids,title,allow_add_to_canvas,version,expires_at,revoked_at) ON genui.live_work_shares TO genui_data;
GRANT SELECT(account_id,organization_id,share_id,created_at,removed_at,grant_version,placement_generation),INSERT(account_id,organization_id,share_id,created_at,removed_at,grant_version),UPDATE(removed_at,grant_version,placement_generation,created_at) ON genui_identity.linked_apps TO genui_workspace;
GRANT SELECT(account_id,event_id,sequence,organization_id,share_id,kind,share_version,canonical,previous_hash,hash,occurred_at),INSERT(account_id,event_id,sequence,organization_id,share_id,kind,share_version,canonical,previous_hash,hash,occurred_at) ON genui_identity.share_events TO genui_workspace;

ALTER POLICY workspace_chain ON genui.audit_events WITH CHECK(writer_role='workspace' AND kind=ANY(ARRAY['session.signed_in','app.created','definition.drafted','work.saved','work.bound','box.created','box.renamed','box.file_added','box.data_registered','box.app_derived','box.shared','box.explained','box.briefed','app.labeled','app.aliased','card.made','answer.pinned','card.shared','card.removed','share.created','share.updated','share.revoked']));
RESET ROLE;

-- Bounded account-session maintenance is the only cross-account function.
-- The existing NOLOGIN admit role cannot be assumed by application roles.
SET LOCAL ROLE genui_owner;
GRANT USAGE ON SCHEMA genui_identity TO genui_admit;
GRANT SELECT(account_id,account_epoch,status) ON genui_identity.accounts TO genui_admit;
GRANT SELECT(account_id,session_id,refresh_token_ref,account_epoch,last_seen_at,expires_at,revoked_at),UPDATE(revoked_at) ON genui_identity.shared_sessions TO genui_admit;
GRANT SELECT(flow_id,expires_at,consumed_at),UPDATE(consumed_at) ON genui_identity.shared_auth_flows TO genui_admit;
GRANT DELETE ON genui_identity.shared_sessions,genui_identity.shared_auth_flows TO genui_admit;
CREATE POLICY identity_cleanup ON genui_identity.accounts FOR SELECT TO genui_admit USING(true);
CREATE POLICY identity_cleanup ON genui_identity.shared_sessions TO genui_admit USING(true) WITH CHECK(false);
CREATE POLICY identity_cleanup ON genui_identity.shared_auth_flows TO genui_admit USING(true) WITH CHECK(false);
CREATE FUNCTION genui_private.expire_shared_identity()
 RETURNS TABLE(refresh_token_ref uuid)
 LANGUAGE plpgsql SECURITY DEFINER STRICT VOLATILE PARALLEL UNSAFE
 SET search_path=pg_catalog,pg_temp AS $$
BEGIN
 DELETE FROM genui_identity.shared_auth_flows f WHERE f.flow_id IN
  (SELECT x.flow_id FROM genui_identity.shared_auth_flows x
   WHERE x.expires_at<=clock_timestamp() OR x.consumed_at IS NOT NULL
   ORDER BY x.flow_id LIMIT 100 FOR UPDATE SKIP LOCKED);
 RETURN QUERY
 DELETE FROM genui_identity.shared_sessions s WHERE (s.account_id,s.session_id) IN
  (SELECT x.account_id,x.session_id FROM genui_identity.shared_sessions x
   JOIN genui_identity.accounts a USING(account_id)
   WHERE x.revoked_at IS NOT NULL OR x.expires_at<=clock_timestamp()
    OR x.last_seen_at<=clock_timestamp()-interval '1 hour'
    OR a.status<>'active' OR a.account_epoch<>x.account_epoch
   ORDER BY x.account_id,x.session_id LIMIT 100 FOR UPDATE OF x SKIP LOCKED)
 RETURNING s.refresh_token_ref;
END $$;
REVOKE ALL ON FUNCTION genui_private.expire_shared_identity() FROM PUBLIC;
GRANT EXECUTE ON FUNCTION genui_private.expire_shared_identity() TO genui_workspace;
GRANT CREATE ON SCHEMA genui_private TO genui_admit;
RESET ROLE;
ALTER FUNCTION genui_private.expire_shared_identity() OWNER TO genui_admit;
SET LOCAL ROLE genui_owner;
REVOKE CREATE ON SCHEMA genui_private FROM genui_admit;
RESET ROLE;
