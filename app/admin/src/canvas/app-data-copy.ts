// An explicit upgrade of an existing source-backed app. This creates editable
// records once; the uploaded original and the app/work identity remain intact.
import {isUuid} from './ids';
import {el} from './render';

type Capability = {editable: boolean; canEnable: boolean; reason: string; publication: string; version: number;
  rows: number; files: {name: string; rows: number}[]; rowLimit: number; byteLimit: number};
type Prepared = {capability: Capability; body: string};
export type AppDataCopy = {show(): void; hide(): void; dispose(): void};
type Options = {app: string; work: string; onEnabled: () => Promise<boolean>};
const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const natural = (value: unknown): value is number => Number.isSafeInteger(value) && Number(value) >= 0;
const positive = (value: unknown): value is number => natural(value) && value > 0;
const text = (value: unknown, limit = 8192): value is string => typeof value === 'string' && value.length <= limit;

function readCapability(value: unknown, app: string, work: string): Capability | null {
  if (!object(value) || value.app_id !== app || value.work_id !== work || typeof value.editable !== 'boolean'
    || typeof value.can_enable !== 'boolean' || !isUuid(value.publication_id) || !positive(value.work_version)
    || !natural(value.total_rows) || !Array.isArray(value.files) || value.files.length > 20
    || !object(value.limits) || !positive(value.limits.rows) || !positive(value.limits.bytes)
    || (value.reason !== undefined && !text(value.reason, 200))) return null;
  const files: Capability['files'] = [];
  for (const file of value.files) {
    if (!object(file) || !text(file.name) || !natural(file.rows)) return null;
    files.push({name: file.name, rows: file.rows});
  }
  if (files.reduce((sum, file) => sum + file.rows, 0) !== value.total_rows
    || (value.can_enable && (value.editable || !files.length || value.total_rows > value.limits.rows))) return null;
  return {editable: value.editable, canEnable: value.can_enable, reason: value.reason as string || '',
    publication: value.publication_id, version: value.work_version, rows: value.total_rows, files,
    rowLimit: value.limits.rows, byteLimit: value.limits.bytes};
}

function idempotencyKey(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('');
}

export function mountAppDataCopy(host: HTMLElement, options: Options): AppDataCopy {
  const doc = host.ownerDocument, url = `/v1/work/${options.app}/${options.work}/data-copy`;
  const status = el(doc, 'p', {class: 'cv-app-data-copy-status', role: 'status', 'aria-live': 'polite'});
  const content = el(doc, 'div', {class: 'cv-app-data-copy-content'});
  const root = el(doc, 'section', {class: 'cv-app-data-copy'}, status, content); host.replaceChildren(root);
  let alive = true, visible = false, verified = false, saving = false, uncertain = false, generation = 0;
  let capability: Capability | null = null, prepared: Prepared | null = null, pending: AbortController | null = null;
  const current = (at: number) => alive && visible && host.isConnected && at === generation;
  const stopRead = () => {generation++; pending?.abort(); pending = null;};
  const clear = () => {capability = null; prepared = null; uncertain = false; verified = false; content.replaceChildren();};
  const finish = async () => {
    prepared = null; uncertain = false; status.textContent = 'App data is enabled. The original files are unchanged.';
    content.replaceChildren();
    const refreshed = await options.onEnabled().catch(() => false);
    if (alive && !refreshed) status.textContent = 'App data is enabled. Refresh the app to load its actions and views. The original files are unchanged.';
  };
  const paint = () => {
    content.replaceChildren(); content.hidden = !verified || !!pending;
    if (!verified || !capability || pending) return;
    if (prepared) {
      const snapshot = prepared;
      const review = el(doc, 'section', {class: 'cv-app-data-copy-review', tabindex: '-1', 'aria-label': 'Review editable app data'},
        el(doc, 'h4', {}, 'Enable app data'),
        el(doc, 'p', {}, `Copy all ${snapshot.capability.rows.toLocaleString()} rows into this app so you can add and update records.`),
        el(doc, 'ul', {}, ...snapshot.capability.files.map(file => el(doc, 'li', {}, `${file.name} · ${file.rows.toLocaleString()} rows`))),
        el(doc, 'p', {}, 'The original files stay unchanged. Existing app views will use the editable copy. Nothing changes until you confirm.'));
      const confirm = el(doc, 'button', {type: 'button', class: 'cv-app-data-copy-confirm'}, uncertain ? 'Retry enabling app data' : 'Confirm and enable');
      const cancel = el(doc, 'button', {type: 'button', class: 'cv-secondary cv-app-data-copy-cancel'}, uncertain ? 'Dismiss' : 'Cancel');
      confirm.disabled = saving || !capability.canEnable; cancel.disabled = saving;
      confirm.addEventListener('click', () => {void commit(snapshot);});
      cancel.addEventListener('click', () => {
        if (!alive || !visible || saving || prepared !== snapshot) return;
        const unresolved = uncertain; prepared = null; uncertain = false;
        status.textContent = unresolved ? 'The result is unconfirmed. Refresh to check whether app data was enabled.' : 'Cancelled. No app data was changed.';
        if (unresolved) {verified = false; void load();} else paint();
      });
      review.append(el(doc, 'div', {class: 'cv-app-chat-actions'}, confirm, cancel)); content.append(review);
      return;
    }
    if (capability.editable) {content.append(el(doc, 'p', {}, 'This app already has editable data. Refresh actions to use it.')); return;}
    if (!capability.canEnable) {
      const limit = capability.reason === 'data_copy_limit';
      content.append(el(doc, 'p', {}, limit
        ? `This data exceeds the editable-copy limit of ${capability.rowLimit.toLocaleString()} rows or ${Math.floor(capability.byteLimit / 1048576)} MiB. The full original is still available to view.`
        : 'Editable app data cannot be enabled for this app. You can still view and ask about its original files.'));
      return;
    }
    const enable = el(doc, 'button', {type: 'button', class: 'cv-app-data-copy-enable'}, 'Enable app data');
    enable.addEventListener('click', () => {
      if (!alive || !visible || !verified || !capability?.canEnable || saving || pending) return;
      prepared = {capability, body: JSON.stringify({publication_id: capability.publication, expected_work_version: capability.version, idempotency_key: idempotencyKey()})};
      status.textContent = ''; paint(); content.querySelector<HTMLElement>('.cv-app-data-copy-review')?.focus({preventScroll: true});
    });
    content.append(el(doc, 'p', {}, 'This app currently reads its original files. Enable an editable copy to add or update its data.'), enable);
  };
  const load = async () => {
    if (!alive || !visible || saving) return;
    if (!isUuid(options.app) || !isUuid(options.work)) {clear(); status.textContent = 'This app is not available.'; return;}
    stopRead(); verified = false; pending = new AbortController(); const controller = pending, at = generation;
    status.textContent = 'Checking whether this app can have editable data…'; paint();
    try {
      const response = await fetch(url, {credentials: 'same-origin', cache: 'no-store', signal: AbortSignal.any([controller.signal, AbortSignal.timeout(15000)])});
      if (!current(at)) return;
      if ([401, 403, 404].includes(response.status)) {clear(); status.textContent = 'Editable app data is not available to you for this app.'; return;}
      if (!response.ok) throw new Error('capability');
      const next = readCapability(await response.json(), options.app, options.work);
      if (!current(at)) return;
      if (!next) throw new Error('capability');
      capability = next; verified = true; status.textContent = '';
      if (next.editable && prepared) {await finish(); return;}
      if (prepared && !uncertain && (prepared.capability.publication !== next.publication || prepared.capability.version !== next.version || !next.canEnable)) {
        prepared = null; status.textContent = 'This app changed. Review its current data before enabling editing.';
      }
    } catch {if (current(at)) status.textContent = 'Editable data could not be checked. Refresh actions to try again.';}
    finally {if (current(at)) {pending = null; paint();}}
  };
  const commit = async (snapshot: Prepared) => {
    if (!alive || !visible || saving || pending || !verified || prepared !== snapshot || !capability?.canEnable) return;
    saving = true; status.textContent = 'Creating the editable app data…'; paint();
    try {
      // A confirmed write continues when the panel closes. Its exact request
      // remains available for an idempotent retry if the response is unknown.
      const response = await fetch(url, {method: 'POST', credentials: 'same-origin', cache: 'no-store', headers: {'content-type': 'application/json'},
        body: snapshot.body, signal: AbortSignal.timeout(120000)});
      const body: unknown = await response.json().catch(() => null);
      if (!alive || prepared !== snapshot) return;
      if (response.ok) {
        const next = readCapability(body, options.app, options.work);
        if (!next?.editable) throw new Error('unconfirmed');
        capability = next; verified = visible; await finish();
      } else if ([401, 403, 404].includes(response.status)) {clear(); status.textContent = 'This change is no longer available to you.';}
      else if (response.status === 409 || response.status === 412) {prepared = null; verified = false; status.textContent = 'This app changed. Refresh actions and review again before enabling app data.';}
      else if (response.status === 422 || response.status === 400) {prepared = null; verified = false; status.textContent = 'This data could not be copied safely. The original files and app are unchanged. Refresh actions to check its current limits.';}
      else throw new Error('unconfirmed');
    } catch {
      if (alive && prepared === snapshot) {uncertain = true; status.textContent = 'The result could not be confirmed. Retry uses the same request and will not import the rows twice.';}
    } finally {if (alive) {saving = false; paint();}}
  };
  root.hidden = true;
  return {
    show() {if (!alive || visible) return; visible = true; root.hidden = false; if (!saving) void load();},
    hide() {visible = false; verified = false; root.hidden = true; stopRead(); paint();},
    dispose() {alive = false; visible = false; stopRead(); clear(); root.remove();},
  };
}
