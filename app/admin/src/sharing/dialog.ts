import { createShare, listShares, ShareError, sharingRequest, sharesPath, type ShareDraft, type ShareSummary } from './api';

function node<K extends keyof HTMLElementTagNameMap>(document: Document, tag: K, text = '', attrs: Record<string, string> = {}): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  element.textContent = text;
  for (const [name, value] of Object.entries(attrs)) element.setAttribute(name, value);
  return element;
}
let current: HTMLDialogElement | null = null;
let scrollLock: { document: Document; overflow: string } | null = null;
export function dismissSharing(): boolean {
  if (!current) return false;
  current.close();
  return true;
}

/** Named access only. The backend is the authority for every mutation. */
export function openSharing(document: Document, app: string, work: string, title: string): void {
  let path: string;
  try { path = sharesPath(app, work); } catch { return; }
  current?.close();
  const returnFocus = document.activeElement as HTMLElement | null;
  if (!scrollLock) scrollLock = { document, overflow: document.documentElement.style.overflow };
  document.documentElement.style.overflow = 'hidden';
  const dialog = node(document, 'dialog', '', { class: 'share-dialog', 'aria-labelledby': 'share-title' });
  current = dialog;
  const top = node(document, 'header', '', { class: 'share-dialog-head' });
  const heading = node(document, 'h2', 'Share this app', { id: 'share-title' });
  const close = node(document, 'button', 'Close', { type: 'button', class: 'share-close', 'aria-label': 'Close sharing' });
  close.addEventListener('click', () => dialog.close());
  top.append(heading, close);
  const subtitle = node(document, 'p', title, { class: 'share-app-title' });
  const policy = node(document, 'p', 'Private link · Live data · View only', { class: 'share-policy' });
  const intro = node(document, 'p', 'Only the named recipient can open this app after signing in. They see live updates to all views and records in this app. Changing its definition or data-source connections pauses this share.', { class: 'share-description' });
  const form = node(document, 'form', '', { class: 'share-create' });
  const emailLabel = node(document, 'label', 'Recipient email');
  const email = node(document, 'input', '', { type: 'email', name: 'recipient_email', required: '', autocomplete: 'email', maxlength: '254', placeholder: 'colleague@example.com' });
  emailLabel.append(email);
  const expiryLabel = node(document, 'label', 'Access expires in');
  const days = node(document, 'select', '', { name: 'expires_in_days' });
  for (const value of [1, 7, 14, 30]) { const option = node(document, 'option', `${value} ${value === 1 ? 'day' : 'days'}`, { value: String(value) }); option.selected = value === 7; days.append(option); }
  expiryLabel.append(days);
  const addLabel = node(document, 'label', '', { class: 'share-check' });
  const allow = node(document, 'input', '', { type: 'checkbox', name: 'allow_add_to_canvas' });
  addLabel.append(allow, node(document, 'span', 'Allow adding this app to their canvas'));
  const help = node(document, 'p', 'Adding creates a connected view. Your app and its data stay with you.', { class: 'share-help' });
  const submit = node(document, 'button', 'Share app', { type: 'submit', class: 'share-primary' });
  const status = node(document, 'p', '', { role: 'status', 'aria-live': 'polite', class: 'share-status' });
  form.append(emailLabel, expiryLabel, addLabel, help, submit, status);
  const list = node(document, 'div', '', { class: 'share-people', 'aria-label': 'People you have shared with' });
  const refresh = node(document, 'button', 'Refresh access list', { type: 'button', class: 'share-secondary' });
  dialog.append(top, subtitle, policy, intro, form, node(document, 'h3', 'People you’ve shared with'), list, refresh);
  document.body.append(dialog);
  dialog.addEventListener('close', () => {
    dialog.remove();
    if (current === dialog) {
      current = null;
      if (scrollLock) scrollLock.document.documentElement.style.overflow = scrollLock.overflow;
      scrollLock = null; returnFocus?.focus();
    }
  }, { once: true });
  const message = (error: unknown) => error instanceof Error ? error.message : 'Sharing could not be updated.';
  let draft: ShareDraft | null = null;
  let busy = false;
  const freeze = (value: boolean) => { email.disabled = value; days.disabled = value; allow.disabled = value; };
  async function load() {
    refresh.disabled = true;
    try {
      const shares = await listShares(path);
      if (!dialog.isConnected) return;
      list.replaceChildren();
      if (!shares.length) list.append(node(document, 'p', 'No named recipients yet.', { class: 'share-empty' }));
      for (const share of shares) list.append(person(share));
    } catch (error) { status.textContent = message(error); }
    finally { refresh.disabled = false; }
  }
  function person(share: ShareSummary): HTMLElement {
    const row = node(document, 'section', '', { class: 'share-person' });
    const detail = node(document, 'div');
    detail.append(node(document, 'strong', share.recipient_email), node(document, 'p', `${share.status} · Expires ${new Date(share.expires_at).toLocaleDateString()}`, { class: 'share-help' }));
    row.append(detail);
    if (share.status === 'revoked' || share.status === 'expired') return row;
    const actions = node(document, 'div', '', { class: 'share-person-actions' });
    const copy = node(document, 'button', 'Copy private link', { type: 'button', class: 'share-secondary' });
    copy.addEventListener('click', async () => {
      try { await navigator.clipboard.writeText(new URL(share.href, location.origin).href); status.textContent = 'Private link copied. Only this recipient can use it.'; }
      catch { status.replaceChildren(node(document, 'a', 'Open private link', { href: share.href })); }
    });
    const toggle = node(document, 'button', share.allow_add_to_canvas ? 'Remove canvas permission' : 'Allow on their canvas', { type: 'button', class: 'share-secondary' });
    const revoke = node(document, 'button', 'Remove access', { type: 'button', class: 'share-danger' });
    const mutate = async (method: string, body: unknown) => {
      toggle.disabled = true; revoke.disabled = true;
      try { await sharingRequest(`${path}/${share.share_id}`, method, body); status.textContent = 'Access updated.'; await load(); }
      catch (error) { status.textContent = message(error); await load(); }
      finally { toggle.disabled = false; revoke.disabled = false; }
    };
    toggle.addEventListener('click', () => void mutate('PATCH', { expected_version: share.version, allow_add_to_canvas: !share.allow_add_to_canvas }));
    revoke.addEventListener('click', () => void mutate('DELETE', { expected_version: share.version }));
    actions.append(copy, toggle, revoke); row.append(actions); return row;
  }
  form.addEventListener('submit', async event => {
    event.preventDefault();
    if (busy || !form.reportValidity()) return;
    draft ??= { recipient_email: email.value.trim(), allow_add_to_canvas: allow.checked, expires_in_days: Number(days.value), idempotency_key: crypto.randomUUID() };
    busy = true; freeze(true); submit.disabled = true; status.textContent = 'Creating private access…';
    try {
      await createShare(path, draft);
      draft = null; email.value = ''; allow.checked = false; days.value = '7'; freeze(false);
      submit.textContent = 'Share app'; status.textContent = 'Private access created. Copy the recipient’s link below.';
      await load();
    } catch (error) {
      status.textContent = message(error);
      if (!(error instanceof ShareError) || !error.uncertain) { draft = null; freeze(false); }
      submit.textContent = draft ? 'Retry same share' : 'Share app';
    } finally { busy = false; submit.disabled = false; }
  });
  refresh.addEventListener('click', () => void load());
  dialog.showModal(); email.focus(); void load();
}

export function startSharing(document: Document): void {
  document.addEventListener('click', event => {
    const trigger = (event.target as Element | null)?.closest<HTMLElement>('[data-manage-share]');
    if (trigger) openSharing(document, trigger.dataset.app ?? '', trigger.dataset.work ?? '', trigger.dataset.title ?? 'App');
  });
}
