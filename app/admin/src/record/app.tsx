import { h, mount, createList } from '@getforma/core';

// APP-013 record page (design §9 record commands, ADR-007 D7.16): the record
// as the member may currently see it and one declared command's form. The
// form carries a per-render idempotency key, the synchronizer token and the
// record's premise (publication, work version, record version) as hidden
// fields; every value is typed and authorized by the server. It reads nothing
// from the request and runs no JavaScript.
type FieldRow = {
  id: string;
  kind: string;
  value: string;
  input: string;
  spec: string;
  specInput: string;
  options: string;
  nullable: boolean;
  writable: boolean;
};
type RecordProps = {
  app: string;
  work: string;
  collection: string;
  recordId: string;
  schema: string;
  schemaVersion: string;
  publication: string;
  workVersion: string;
  version: string;
  exists: boolean;
  fields: FieldRow[];
  command: string;
  action: string;
  canWrite: boolean;
  csrf: string;
  idempotencyKey: string;
  committed: boolean;
  replayed: boolean;
  conflict: boolean;
  stale: boolean;
  inFlight: boolean;
  busy: boolean;
  refused: boolean;
  unknown: boolean;
};

function Record(props: RecordProps) {
  return (
    <main class="page">
      <h1>Record</h1>
      <p class="lede">
        Saved app. Schema <span>{props.schema}</span> version{' '}
        <span>{props.schemaVersion}</span>; record <span>{props.recordId}</span>.
      </p>
      {props.committed ? (
        <p class="notice" role="status">The command was committed. The record is shown at its new version.</p>
      ) : null}
      {props.replayed ? (
        <p class="notice" role="status">This command had already been committed; its stored outcome was returned.</p>
      ) : null}
      {props.conflict ? (
        <p class="notice" role="alert">
          The record changed before your command: it is shown as it is now. Review it and submit again.
        </p>
      ) : null}
      {props.stale ? (
        <p class="notice" role="alert">The saved work moved to a newer revision. The record is shown under it.</p>
      ) : null}
      {props.inFlight ? (
        <p class="notice" role="alert">The same command is still being committed. Reload in a moment.</p>
      ) : null}
      {props.busy ? (
        <p class="notice" role="alert">The record was busy. Reload in a moment and submit again.</p>
      ) : null}
      {props.refused ? (
        <p class="notice" role="alert">
          The command was refused: a value is not one this command accepts. Nothing was changed.
        </p>
      ) : null}
      {props.unknown ? (
        <p class="notice" role="alert">
          The result of your last command could not be confirmed. The record is shown as it is now.
        </p>
      ) : null}
      {props.exists ? (
        <p>
          Record version <span>{props.version}</span>; work version <span>{props.workVersion}</span>.
        </p>
      ) : (
        <p>
          No record committed yet (version <span>{props.version}</span>); work version{' '}
          <span>{props.workVersion}</span>.
        </p>
      )}
      <table class="record">
        <tbody>
          {createList(
            () => props.fields,
            (row) => row.id,
            (row) =>
              h(
                'tr',
                null,
                h('th', { scope: 'row' }, row.id),
                h('td', null, row.value),
                h('td', { class: 'muted' }, row.kind),
              ),
          )}
        </tbody>
      </table>
      {props.canWrite ? (
        <form method="post" action={props.action} class="command">
          <input type="hidden" name="csrf" value={props.csrf} />
          <input type="hidden" name="idempotency_key" value={props.idempotencyKey} />
          <input type="hidden" name="collection_id" value={props.collection} />
          <input type="hidden" name="publication_id" value={props.publication} />
          <input type="hidden" name="schema_id" value={props.schema} />
          <input type="hidden" name="schema_version_expected" value={props.schemaVersion} />
          <input type="hidden" name="record_id" value={props.recordId} />
          <input type="hidden" name="expected_record_version" value={props.version} />
          <input type="hidden" name="expected_work_version" value={props.workVersion} />
          {createList(
            () => props.fields,
            (row) => row.id,
            // Only the offered command's writable fields are inputs; storage
            // refuses a value for any other field (APP-013 review m3).
            (row) =>
              h(
                'p',
                { class: 'field' },
                h('label', null, row.id, ' ', h('span', { class: 'muted' }, row.options)),
                row.writable
                  ? h('input', { type: 'text', name: row.input, value: row.value })
                  : h('span', { class: 'muted' }, 'not changed by this command'),
                row.writable ? h('input', { type: 'hidden', name: row.specInput, value: row.spec }) : null,
              ),
          )}
          <button type="submit">Submit {props.command}</button>
        </form>
      ) : (
        <p class="muted">You can read this record but not change it.</p>
      )}
      <p><a href="/workspace">Back to the workspace</a></p>
    </main>
  );
}

mount(
  () =>
    Record({
      app: '',
      work: '',
      collection: '',
      recordId: '',
      schema: '',
      schemaVersion: '',
      publication: '',
      workVersion: '',
      version: '',
      exists: false,
      fields: [],
      command: '',
      action: '',
      canWrite: false,
      csrf: '',
      idempotencyKey: '',
      committed: false,
      replayed: false,
      conflict: false,
      stale: false,
      inFlight: false,
      busy: false,
      refused: false,
      unknown: false,
    }),
  '#app',
);
