// Prepare a change inside one app, then explicitly confirm its existing
// audited command. Neither the builder nor the global action router is used.
import { postCommand, type CommandOutcome } from './commands';
import {mountAppDataCopy, type AppDataCopy} from './app-data-copy';
import { isUuid } from './ids';
import { el } from './render';

type Field = {id: string; type: string};
type Action = {id: string; operation: 'create' | 'update'; fields: string[]; collection: string; schema: string; schemaVersion: number; knownFields: Field[]};
type RecordItem = {id: string; collection: string; version: number; values: {field: string; display: string}[]};
type Catalog = {publication: string; version: number; canWrite: boolean; actions: Action[]; records: RecordItem[]; truncated: boolean};
type Proposal = {action: Action; record: string; recordVersion: number; recordLabel: string; description: string; publication: string; workVersion: number;
  values: {field: string; label: string; display: string; previous?: string}[]; serialized: string; key: string};
export type AppDataActions = {show(): void; hide(): void; dispose(): void; focus(): void};
export type AppDataActionsOptions = {app: string; work: string; title: string; onCommitted?: (recordId: string) => void; onOpenActions?: () => void; onDataEnabled?: () => Promise<boolean>};
const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const localId = (value: unknown): value is string => typeof value === 'string' && /^[a-z][a-z0-9_]{0,63}$/.test(value);
const positive = (value: unknown): value is number => Number.isSafeInteger(value) && Number(value) > 0;
const text = (value: unknown, limit = 8192): value is string => typeof value === 'string' && value.length <= limit;
const label = (value: string) => value.replace(/_/g, ' ').replace(/^./, first => first.toUpperCase());
const actionKey = (action: Action) => `${action.collection}/${action.id}`;
const unavailable = 'This app’s actions are no longer available to you.';

function readCatalog(value: unknown, app: string, work: string): Catalog | null {
  if (!object(value) || value.app_id !== app || value.work_id !== work || !isUuid(value.publication_id) || !positive(value.work_version)
    || typeof value.can_write !== 'boolean' || !Array.isArray(value.collections) || value.collections.length > 64
    || !Array.isArray(value.records) || value.records.length > 5000 || typeof value.truncated !== 'boolean') return null;
  const actions: Action[] = [], records: RecordItem[] = [], collections = new Set<string>();
  for (const collection of value.collections) {
    if (!object(collection) || !isUuid(collection.collection_id) || !localId(collection.schema_id) || !positive(collection.schema_version)
      || !Array.isArray(collection.fields) || collection.fields.length > 128 || !Array.isArray(collection.commands) || collection.commands.length > 128
      || collections.has(collection.collection_id)) return null;
    collections.add(collection.collection_id);
    const fields: Field[] = [];
    for (const field of collection.fields) {
      if (!object(field) || !localId(field.id) || !text(field.type, 40) || fields.some(known => known.id === field.id)) return null;
      fields.push({id: field.id, type: field.type});
    }
    for (const command of collection.commands) {
      if (!object(command) || !localId(command.id) || !text(command.operation, 40)) return null;
      if (command.operation !== 'create' && command.operation !== 'update') continue;
      const transitions = command.transitions ?? [];
      if (!Array.isArray(transitions) || transitions.length > 128 || !transitions.every(transition => object(transition)
        && localId(transition.field_id) && text(transition.from, 2000) && text(transition.to, 2000))) return null;
      if (!Array.isArray(command.writable_fields) || command.writable_fields.length > 32
        || !command.writable_fields.every(id => localId(id) && fields.some(field => field.id === id))
        || new Set(command.writable_fields).size !== command.writable_fields.length) return null;
      if (actions.some(action => action.collection === collection.collection_id && action.id === command.id)) return null;
      // Older published decision commands say "create" but their transitions
      // act on an existing record, just like the native records controls.
      const operation = command.operation === 'update' || transitions.length ? 'update' : 'create';
      actions.push({id: command.id, operation, fields: command.writable_fields as string[], collection: collection.collection_id,
        schema: collection.schema_id, schemaVersion: collection.schema_version, knownFields: fields});
    }
  }
  for (const record of value.records) {
    if (!object(record) || !isUuid(record.record_id) || !isUuid(record.collection_id) || !collections.has(record.collection_id) || !positive(record.version)
      || !Array.isArray(record.values) || record.values.length > 128 || records.some(known => known.id === record.record_id)) return null;
    const values: RecordItem['values'] = [];
    for (const field of record.values) {
      if (!object(field) || !localId(field.field_id) || !text(field.display)
        || (field.assistant_display !== undefined && !text(field.assistant_display)) || values.some(known => known.field === field.field_id)) return null;
      values.push({field: field.field_id, display: typeof field.assistant_display === 'string' ? field.assistant_display : field.display});
    }
    records.push({id: record.record_id, collection: record.collection_id, version: record.version, values});
  }
  return {publication: value.publication_id, version: value.work_version, canWrite: value.can_write, actions, records, truncated: value.truncated};
}

function readProposal(value: unknown, app: string, work: string, catalog: Catalog, action: Action, target: RecordItem | null, description: string): Proposal | null {
  if (!object(value) || value.kind !== 'app_action_proposal' || value.app_id !== app || value.work_id !== work
    || value.operation !== action.operation || value.command_id !== action.id || value.collection_id !== action.collection || !isUuid(value.record_id)
    || (target ? value.record_id !== target.id : catalog.records.some(record => record.id === value.record_id))
    || !Array.isArray(value.values) || !value.values.length || value.values.length > 32 || !object(value.command_request)) return null;
  const request = value.command_request;
  if (request.kind !== 'command_request' || request.schema_version !== 1 || request.app_id !== app || request.work_id !== work
    || request.publication_id !== catalog.publication || request.command_id !== action.id || request.schema_id !== action.schema
    || request.schema_version_expected !== action.schemaVersion || request.record_id !== value.record_id
    || request.expected_record_version !== (target?.version ?? 0) || request.expected_work_version !== catalog.version
    || typeof request.idempotency_key !== 'string' || !/^[0-9a-f]{32}$/.test(request.idempotency_key)
    || !Array.isArray(request.values) || request.values.length !== value.values.length) return null;
  const expectedKeys = ['kind', 'schema_version', 'app_id', 'work_id', 'publication_id', 'command_id', 'schema_id', 'schema_version_expected', 'record_id', 'expected_record_version', 'expected_work_version', 'values', 'idempotency_key'];
  if (Object.keys(request).some(key => !expectedKeys.includes(key))) return null;
  const values: Proposal['values'] = [], seen = new Set<string>();
  for (const field of value.values) {
    if (!object(field) || !localId(field.field_id) || !action.fields.includes(field.field_id) || seen.has(field.field_id)
      || !text(field.label, 200) || !text(field.display) || !object(field.value) || JSON.stringify(field.value).length > 8192
      || (field.previous !== undefined && (!target || !text(field.previous)))) return null;
    const submitted = request.values.filter(candidate => object(candidate) && candidate.field_id === field.field_id);
    if (submitted.length !== 1 || !object(submitted[0]) || Object.keys(submitted[0]).sort().join(',') !== 'field_id,value'
      || JSON.stringify(submitted[0].value) !== JSON.stringify(field.value)) return null;
    seen.add(field.field_id);
    values.push({field: field.field_id, label: field.label, display: field.display, ...(typeof field.previous === 'string' ? {previous: field.previous} : {})});
  }
  return {action, record: value.record_id, recordVersion: target?.version ?? 0, description, publication: catalog.publication, workVersion: catalog.version,
    recordLabel: target?.values.map(field => field.display).filter(Boolean).slice(0, 3).join(' · ').slice(0, 200) || 'Selected record',
    values, serialized: JSON.stringify(request), key: request.idempotency_key};
}

export function mountAppDataActions(host: HTMLElement, options: AppDataActionsOptions): AppDataActions {
  const doc = host.ownerDocument;
  const actionInput = el(doc, 'select', {'aria-label': 'App action', class: 'cv-app-action-select'});
  const recordInput = el(doc, 'select', {'aria-label': 'Record to change', class: 'cv-app-action-record'});
  const recordLabel = el(doc, 'label', {}, el(doc, 'span', {}, 'Record to change'), recordInput);
  const searchInput = el(doc, 'input', {type: 'search', class: 'cv-app-action-search-input', maxlength: '200', placeholder: 'Find by text or record ID…', 'aria-label': 'Find an app record'});
  const searchButton = el(doc, 'button', {type: 'button', class: 'cv-secondary cv-app-action-search-button'}, 'Find records');
  const searchStatus = el(doc, 'p', {class: 'cv-app-action-search-status', role: 'status'});
  const recordPicker = el(doc, 'div', {class: 'cv-app-action-record-picker'}, recordLabel,
    el(doc, 'div', {class: 'cv-app-chat-actions cv-app-action-search'}, searchInput, searchButton), searchStatus);
  const description = el(doc, 'textarea', {class: 'cv-app-action-description', rows: '4', maxlength: '2000', placeholder: 'Describe the values to add or change…', 'aria-label': 'Describe the data change'});
  const prepare = el(doc, 'button', {type: 'submit', class: 'cv-app-action-prepare'}, 'Review change');
  const refresh = el(doc, 'button', {type: 'button', class: 'cv-secondary cv-app-action-refresh'}, 'Refresh actions');
  const status = el(doc, 'p', {class: 'cv-app-action-status', role: 'status', 'aria-live': 'polite'});
  const result = el(doc, 'div', {class: 'cv-app-action-result', 'aria-live': 'polite'});
  const review = el(doc, 'section', {class: 'cv-app-action-review', 'aria-label': 'Review proposed data change', tabindex: '-1'});
  const copyHost = el(doc, 'div', {class: 'cv-app-data-copy-host'});
  const form = el(doc, 'form', {class: 'cv-app-action-form'}, el(doc, 'label', {}, el(doc, 'span', {}, 'Action'), actionInput), recordPicker,
    el(doc, 'label', {}, el(doc, 'span', {}, 'What should change?'), description), prepare);
  const root = el(doc, 'section', {class: 'cv-app-data-actions'},
    el(doc, 'p', {class: 'cv-app-action-intro'}, 'Use this app’s existing actions to add or update data. Review the proposed values before saving.'), status, refresh, copyHost, form, review, result);
  if (options.onOpenActions) {
    const native = el(doc, 'button', {type: 'button', class: 'cv-secondary cv-app-action-native'}, 'Open app forms');
    native.addEventListener('click', () => {if (alive && visible && catalogVerified && catalog) options.onOpenActions?.();}); root.append(native);
  }
  host.replaceChildren(root);
  let alive = true, visible = false, generation = 0, pending: AbortController | null = null;
  let catalog: Catalog | null = null, proposal: Proposal | null = null, saving = false, uncertain = false, catalogVerified = false;
  let dataCopy: AppDataCopy | null = null;
  const currentAction = () => catalog?.actions.find(action => actionKey(action) === actionInput.value);
  const currentRecord = () => catalog?.records.find(record => record.id === recordInput.value && record.collection === currentAction()?.collection) ?? null;
  const current = (ticket: number) => alive && visible && host.isConnected && generation === ticket;
  const cancelRead = () => {generation++; pending?.abort(); pending = null;};
  const clearSensitive = () => {catalog = null; catalogVerified = false; proposal = null; uncertain = false; description.value = ''; searchInput.value = ''; searchStatus.textContent = ''; result.replaceChildren(); actionInput.replaceChildren(); recordInput.replaceChildren(); dataCopy?.hide();};
  const controls = () => {
    form.hidden = !catalogVerified || !catalog?.canWrite || !catalog.actions.length || !!proposal;
    copyHost.hidden = !catalogVerified || !catalog || !!catalog.actions.length;
    review.hidden = !catalogVerified || !proposal; result.hidden = !catalogVerified;
    actionInput.disabled = saving || uncertain || !catalogVerified; recordInput.disabled = saving || uncertain || !catalogVerified; description.disabled = saving || uncertain || !catalogVerified;
    searchInput.disabled = saving || uncertain || !catalogVerified || !!pending; searchButton.disabled = searchInput.disabled || !!proposal;
    prepare.disabled = !!pending || saving || !!proposal || !currentAction() || (currentAction()?.operation === 'update' && !currentRecord());
    refresh.hidden = !!pending || saving || (!!proposal && catalogVerified);
  };
  const describeRecord = (record: RecordItem) => `${record.values.map(value => value.display).filter(Boolean).slice(0, 3).join(' · ').slice(0, 160) || record.id.slice(0, 8)} · v${record.version}`;
  const populateRecords = () => {
    const selected = recordInput.value, action = currentAction();
    recordPicker.hidden = action?.operation !== 'update';
    recordInput.replaceChildren(el(doc, 'option', {value: ''}, 'Choose a record…'));
    for (const record of catalog?.records.filter(value => value.collection === action?.collection) ?? []) recordInput.append(el(doc, 'option', {value: record.id}, describeRecord(record)));
    recordInput.value = catalog?.records.some(record => record.id === selected && record.collection === action?.collection) ? selected : '';
    if (!searchStatus.textContent && catalog?.truncated) searchStatus.textContent = 'The list shows recent records. Use Find records to select older data.';
  };
  const invalidate = (dismissUncertain = false) => {
    if (saving || (uncertain && !dismissUncertain)) return;
    cancelRead(); proposal = null; uncertain = false; review.replaceChildren(); status.textContent = ''; controls();
  };
  const showReview = () => {
    review.replaceChildren();
    if (!proposal) return;
    const prepared = proposal;
    const title = prepared.action.operation === 'create' ? 'Add a record' : 'Update this record';
    const values = el(doc, 'dl', {class: 'cv-confirm-values'});
    for (const value of prepared.values) values.append(el(doc, 'div', {}, el(doc, 'dt', {}, value.label),
      el(doc, 'dd', {}, ...(value.previous === undefined ? [] : [el(doc, 'span', {class: 'cv-app-action-before'}, value.previous || 'Empty'), ' → ']), el(doc, 'strong', {}, value.display || 'Empty'))));
    const confirm = el(doc, 'button', {type: 'button', class: 'cv-app-action-confirm'}, uncertain ? 'Retry this save' : 'Confirm and save');
    const cancel = el(doc, 'button', {type: 'button', class: 'cv-secondary cv-app-action-cancel'}, uncertain ? 'Dismiss' : 'Cancel');
    confirm.disabled = saving || !!pending || !catalogVerified || !catalog?.canWrite; cancel.disabled = saving;
    confirm.addEventListener('click', () => {void commit(prepared);});
    cancel.addEventListener('click', () => {
      if (!alive || !visible || saving || proposal !== prepared) return;
      const unresolved = uncertain; invalidate(true); status.textContent = unresolved ? 'The save result is still unconfirmed. Check this app before starting another change.' : 'Draft cancelled. No change was saved.';
      description.focus({preventScroll: true});
    });
    review.append(el(doc, 'h4', {}, title), el(doc, 'p', {}, `${label(prepared.action.id)} · ${label(prepared.action.schema)}`),
      ...(prepared.action.operation === 'update' ? [el(doc, 'p', {}, prepared.recordLabel)] : []),
      values, el(doc, 'p', {class: 'cv-app-action-confirm-note'}, saving ? 'Saving your confirmed change…' : uncertain ? 'The previous save could not be confirmed. Retry checks the same submission; it will not create a second change.' : 'Nothing is saved until you confirm.'),
      el(doc, 'div', {class: 'cv-app-chat-actions'}, confirm, cancel));
  };
  const findRecords = async (before: Catalog, action: Action, query: string, signal: AbortSignal): Promise<Catalog> => {
    const response = await fetch(`/v1/work/${options.app}/${options.work}/records/search?${new URLSearchParams({collection: action.collection, q: query})}`, {
      credentials: 'same-origin', cache: 'no-store', signal: AbortSignal.any([signal, AbortSignal.timeout(15000)]),
    });
    if ([401, 403, 404].includes(response.status)) throw new Error('denied');
    if (!response.ok) throw new Error('search');
    const found = readCatalog(await response.json(), options.app, options.work);
    if (!found || found.records.length > 50 || found.records.some(record => record.collection !== action.collection)
      || found.actions.some(value => value.collection !== action.collection)) throw new Error('search');
    if (!found.canWrite) throw new Error('denied');
    if (found.publication !== before.publication || found.version !== before.version) throw new Error('stale');
    return found;
  };
  const load = async () => {
    if (!alive || !visible || saving) return;
    if (!isUuid(options.app) || !isUuid(options.work)) {clearSensitive(); status.textContent = unavailable; showReview(); controls(); return;}
    cancelRead(); catalogVerified = false; pending = new AbortController(); const controller = pending, ticket = generation;
    dataCopy?.hide();
    status.textContent = 'Checking this app’s available actions…'; controls();
    try {
      const response = await fetch(`/v1/work/${options.app}/${options.work}/records`, {credentials: 'same-origin', cache: 'no-store', signal: AbortSignal.any([controller.signal, AbortSignal.timeout(15_000)])});
      if (!current(ticket)) return;
      if ([401, 403, 404].includes(response.status)) {clearSensitive(); status.textContent = unavailable; return;}
      if (!response.ok) throw new Error('catalog');
      const next = readCatalog(await response.json(), options.app, options.work);
      if (!current(ticket)) return;
      if (!next) throw new Error('catalog');
      if (proposal && !uncertain && proposal.action.operation === 'update'
        && next.publication === proposal.publication && next.version === proposal.workVersion
        && next.actions.some(action => actionKey(action) === actionKey(proposal!.action))
        && !next.records.some(record => record.id === proposal!.record)) {
        const found = await findRecords(next, proposal.action, proposal.record, controller.signal);
        if (!current(ticket)) return;
        next.records.push(...found.records.filter(record => record.id === proposal!.record));
      }
      const selected = actionInput.value; catalog = next; catalogVerified = true; searchStatus.textContent = '';
      actionInput.replaceChildren(el(doc, 'option', {value: ''}, 'Choose an action…'));
      for (const action of next.actions) actionInput.append(el(doc, 'option', {value: actionKey(action)}, `${label(action.id)} · ${label(action.schema)} (${action.operation === 'create' ? 'add' : 'update'})`));
      actionInput.value = next.actions.some(action => actionKey(action) === selected) ? selected : next.actions.length === 1 ? actionKey(next.actions[0]!) : '';
      populateRecords();
      if (!next.actions.length) {proposal = null; uncertain = false; status.textContent = 'This app has no supported actions for adding or updating data.';}
      else if (!next.canWrite) {proposal = null; uncertain = false; status.textContent = 'You can view this app’s data, but you do not have permission to change it.';}
      else if (proposal && !uncertain && (proposal.publication !== next.publication || proposal.workVersion !== next.version
        || !next.actions.some(action => actionKey(action) === actionKey(proposal!.action))
        || (proposal.action.operation === 'update' && !next.records.some(record => record.id === proposal!.record && record.version === proposal!.recordVersion)))) {
        proposal = null; status.textContent = 'This app or record changed. Your description is kept; review a new draft.';
      } else status.textContent = '';
    } catch (error) {
      if (current(ticket)) {
        if (error instanceof Error && error.message === 'denied') {clearSensitive(); status.textContent = unavailable;}
        else {catalogVerified = false; status.textContent = 'Actions could not be checked. Refresh to try again.';}
      }
    } finally {
      if (current(ticket)) {
        pending = null; showReview(); controls();
        if (catalogVerified && catalog && !catalog.actions.length) {
          dataCopy ??= mountAppDataCopy(copyHost, {...options, onEnabled: async () => {
            const refreshed = await (options.onDataEnabled?.() ?? Promise.resolve(true));
            if (alive && visible) await load();
            if (alive && visible && !refreshed) status.textContent = 'App data is enabled. Refresh the app to load its updated views. The original files are unchanged.';
            return refreshed;
          }});
          dataCopy.show();
        }
      }
    }
  };
  const search = async () => {
    const before = catalog, action = currentAction(), query = searchInput.value.trim();
    if (!alive || !visible || !catalogVerified || !before?.canWrite || action?.operation !== 'update' || saving || pending || proposal) return;
    if (Array.from(query).length < 2 || Array.from(query).length > 200 || /[\u0000-\u001f\u007f]/.test(query)) {searchStatus.textContent = 'Enter 2 to 200 characters to find a record.'; return;}
    cancelRead(); pending = new AbortController(); const controller = pending, ticket = generation;
    recordInput.value = ''; searchStatus.textContent = 'Finding records in this app…'; controls();
    try {
      const found = await findRecords(before, action, query, controller.signal);
      if (!current(ticket) || currentAction() !== action) return;
      catalog = {...before, records: found.records, truncated: found.truncated}; populateRecords();
      searchStatus.textContent = found.truncated ? 'Showing 50 matches. Refine your search to find the record you want.'
        : found.records.length ? `${found.records.length} ${found.records.length === 1 ? 'match' : 'matches'}. Choose the record to update.` : 'No matching records. Try another detail or the full record ID.';
    } catch (error) {
      if (current(ticket)) {
        if (error instanceof Error && error.message === 'denied') {clearSensitive(); status.textContent = unavailable;}
        else if (error instanceof Error && error.message === 'stale') {catalogVerified = false; status.textContent = 'This app changed. Refresh actions, then search again.';}
        else searchStatus.textContent = 'Records could not be found. Try again.';
      }
    } finally {if (current(ticket)) {pending = null; controls();}}
  };
  const commit = async (prepared: Proposal) => {
    if (!alive || !visible || saving || pending || proposal !== prepared || !catalogVerified || !catalog?.canWrite) return;
    if (!uncertain && (description.value.trim() !== prepared.description || actionInput.value !== actionKey(prepared.action)
      || (prepared.action.operation === 'update' && recordInput.value !== prepared.record))) {invalidate(); status.textContent = 'Your request changed. Review it again before saving.'; return;}
    saving = true; status.textContent = 'Saving your confirmed change…'; showReview(); controls();
    let outcome: CommandOutcome;
    try {
      outcome = await postCommand(`/v1/work/${options.app}/${options.work}/commands/${prepared.action.id}`, JSON.parse(prepared.serialized), prepared.key, {
        conflict: 'This record changed. Review a new draft before saving.', stale: 'This app changed. Review a new draft before saving.',
        refused: 'This change is no longer permitted. Nothing was saved by this submission.',
        unconfirmed: 'The save result could not be confirmed. Retry this same save to check its result.',
      });
    } catch {outcome = {kind: 'unconfirmed', message: 'The save result could not be confirmed. Retry this same save to check its result.'};}
    if (!alive || proposal !== prepared) return;
    saving = false;
    if (outcome.kind === 'committed') {
      proposal = null; uncertain = false;
      result.replaceChildren(el(doc, 'p', {}, prepared.action.operation === 'create' ? 'Record added.' : 'Record updated.'),
        el(doc, 'a', {href: `/work/${options.app}/${options.work}/records/${prepared.action.collection}/${prepared.record}`, target: '_blank', rel: 'noopener'}, 'Open saved record'));
      status.textContent = 'Saved.'; options.onCommitted?.(prepared.record);
      if (description.value.trim() === prepared.description) description.value = '';
      if (visible) void load();
    } else {
      status.textContent = outcome.message;
      if (outcome.kind === 'conflict' || outcome.kind === 'stale' || outcome.kind === 'refused') {proposal = null; uncertain = false; catalog = null;}
      else uncertain = true;
    }
    showReview(); controls();
  };
  form.addEventListener('submit', async event => {
    event.preventDefault();
    const action = currentAction(), before = catalog, target = currentRecord(), words = description.value.trim();
    if (!visible || !alive || !catalogVerified || !before?.canWrite || !action || pending || saving || proposal) return;
    if (words.length < 3 || Array.from(words).length > 2000) {status.textContent = 'Describe the change in 3 to 2,000 characters.'; return;}
    if (action.operation === 'update' && !target) {status.textContent = 'Choose the record to update first.'; return;}
    cancelRead(); pending = new AbortController(); const controller = pending, ticket = generation;
    result.replaceChildren(); status.textContent = 'Preparing values for your review…'; controls();
    try {
      const response = await fetch(`/v1/work/${options.app}/${options.work}/assistant/proposals`, {method: 'POST', credentials: 'same-origin', cache: 'no-store', signal: AbortSignal.any([controller.signal, AbortSignal.timeout(35_000)]),
        headers: {'content-type': 'application/json'}, body: JSON.stringify({command_id: action.id, collection_id: action.collection, utterance: words,
          publication_id: before.publication, expected_work_version: before.version,
          ...(action.operation === 'update' && target ? {record_id: target.id, expected_record_version: target.version} : {})})});
      const body: unknown = await response.json().catch(() => null);
      if (!current(ticket)) return;
      if ([401, 403, 404].includes(response.status)) {clearSensitive(); status.textContent = unavailable; return;}
      if (!response.ok) {
        if (response.status === 409 || response.status === 412) {catalog = null; status.textContent = 'This app or record changed. Refresh actions, then review a new draft.';}
        else if (object(body) && body.code === 'app_data_limit') status.textContent = 'This change would exceed the app data limit of 100,000 rows or 64 MiB. Nothing was saved. Original files are unchanged.';
        else if (object(body) && body.code === 'composer_unavailable') status.textContent = 'The assistant cannot prepare changes right now. You can use the app’s forms.';
        else if (object(body) && Array.isArray(body.missing_fields) && body.missing_fields.every(field => text(field, 200))) status.textContent = `Add these details to your request: ${body.missing_fields.slice(0, 32).join(', ')}.`;
        else status.textContent = 'That request could not be prepared for this action. Check the details and try again.';
        return;
      }
      proposal = readProposal(body, options.app, options.work, before, action, action.operation === 'update' ? target : null, words);
      status.textContent = proposal ? 'Review the proposed values below.' : 'The proposed change did not match this app and action. Nothing was saved.';
    } catch {if (current(ticket)) status.textContent = 'The change could not be prepared. Check your connection and try again.';}
    finally {if (current(ticket)) {pending = null; showReview(); controls(); if (proposal) review.focus({preventScroll: true});}}
  });
  description.addEventListener('input', () => invalidate());
  actionInput.addEventListener('change', () => {invalidate(); searchStatus.textContent = ''; populateRecords(); controls();});
  recordInput.addEventListener('change', () => invalidate());
  refresh.addEventListener('click', () => {void load();});
  searchButton.addEventListener('click', () => {void search();});
  searchInput.addEventListener('keydown', event => {if (event.key === 'Enter') {event.preventDefault(); event.stopPropagation(); void search();}});
  form.addEventListener('keydown', event => {if (event.key !== 'Escape') event.stopPropagation();});
  controls(); root.hidden = true;
  return {
    show() {if (!alive || visible) return; visible = true; root.hidden = false; if (!saving) void load();},
    hide() {visible = false; root.hidden = true; catalogVerified = false; cancelRead(); dataCopy?.hide(); controls();},
    focus() {if (alive && visible && catalog?.canWrite) description.focus();},
    dispose() {alive = false; visible = false; cancelRead(); clearSensitive(); dataCopy?.dispose(); root.remove();},
  };
}
