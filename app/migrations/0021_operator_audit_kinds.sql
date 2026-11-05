-- APP-084: the operator chain gains the provider-invitation and operator-command
-- kinds, and a private index makes one provider tenant belong to one
-- organization. Additive: no genui table changes and no stored value is rewritten.
-- genui_private.provider_tenant_index has no RLS, like code_index and
-- handle_index, because FORCE RLS hides every other organization's binding.
-- Executed only by the checksummed, transactional migration runner (D7.12).
SET LOCAL ROLE genui_owner;

ALTER TABLE genui_meta.schema_version DROP CONSTRAINT schema_version_version_check,
 ADD CONSTRAINT schema_version_version_check CHECK (version BETWEEN 1 AND 21);
ALTER TABLE genui_meta.schema_migrations DROP CONSTRAINT schema_migrations_version_check,
 ADD CONSTRAINT schema_migrations_version_check CHECK (version BETWEEN 1 AND 21);

ALTER POLICY operator_chain ON genui.audit_events
 WITH CHECK (writer_role = 'operator' AND kind = ANY (ARRAY['organization.provisioned','handle.rotated',
  'code.issued','code.revoked','code.extended','membership.revoked','entitlement.revoked','permission.granted',
  'permission.revoked','creator_authority.revoked','provider.invitation.created','provider.invitation.reconciled',
  'provider.invitation.revoked','source_revision.registered','work.seeded','provider_tenant.bound','app.seeded']));

CREATE TABLE genui_private.provider_tenant_index (
 issuer text NOT NULL,
 provider_tenant_id text NOT NULL,
 organization_id uuid NOT NULL REFERENCES genui.organizations,
 PRIMARY KEY (issuer, provider_tenant_id)
);
ALTER TABLE genui_private.provider_tenant_index OWNER TO genui_owner;

-- Copy every active binding, including an organization that has neither a
-- handle nor a code. FORCE RLS would hide those rows from genui_owner, so the
-- copy disables RLS for this statement only and turns it back on before commit.
-- Two organizations already sharing one issuer and tenant fail the primary key.
ALTER TABLE genui.provider_tenant_bindings NO FORCE ROW LEVEL SECURITY;
ALTER TABLE genui.provider_tenant_bindings DISABLE ROW LEVEL SECURITY;
INSERT INTO genui_private.provider_tenant_index(issuer, provider_tenant_id, organization_id)
SELECT issuer, provider_tenant_id, organization_id
FROM genui.provider_tenant_bindings
WHERE active;
ALTER TABLE genui.provider_tenant_bindings ENABLE ROW LEVEL SECURITY;
ALTER TABLE genui.provider_tenant_bindings FORCE ROW LEVEL SECURITY;
RESET ROLE;
