import {
  createShare, listShares, offeredRoles, readColleagues, readTeamSharing, revokeMemberShare, ROLE_WORDS, ShareError,
  shareWithEveryone, shareWithMember, sharingRequest, sharesPath, UNAVAILABLE_FOR_EVERYONE, type ShareDraft,
  type ShareSummary, type TeamRole, type TeamSharing,
} from './api';

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
  const team = teamSection(document, app, work);
  const outside = node(document, 'h3', 'Someone outside your organization', { class: 'share-outside' });
  dialog.append(top, subtitle, team.section, outside, policy, intro, form, node(document, 'h3', 'People you’ve shared with'), list, refresh);
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
  // The first control (Close) takes focus, so the dialog opens at its top.
  dialog.showModal(); void load(); void team.load();
}

const RANK: Record<TeamRole, number> = { viewer: 1, contributor: 2, reviewer: 3 };

/** One in-place confirmation: a sentence naming the effect, a yes and a cancel. */
function confirmation(document: Document, onCancel: () => void) {
  const box = node(document, 'div', '', { class: 'share-confirm' });
  const text = node(document, 'p', '', { class: 'share-confirm-text' });
  const yes = node(document, 'button', '', { type: 'button', class: 'share-primary' });
  const no = node(document, 'button', 'Cancel', { type: 'button', class: 'share-secondary' });
  box.append(text, yes, no);
  box.hidden = true;
  let pending: (() => void) | null = null;
  const close = () => { pending = null; box.hidden = true; };
  yes.addEventListener('click', () => { const run = pending; close(); run?.(); });
  no.addEventListener('click', () => { close(); onCancel(); });
  return {
    box, close,
    ask(sentence: string, label: string, run: () => void) { text.textContent = sentence; yes.textContent = label; pending = run; box.hidden = false; },
  };
}

/**
 * APP-192: people in the organization. The server says which roles this
 * sharer may grant (reviewer only for a reviewer) and refuses anything else;
 * the section stays hidden for a member who cannot share the app (the 404),
 * and shows a plain message with a retry when the read fails otherwise.
 * Exported for the dialog tests.
 */
export function teamSection(document: Document, app: string, work: string): { section: HTMLElement; load: () => Promise<void> } {
  const section = node(document, 'section', '', { class: 'share-team', 'aria-labelledby': 'share-team-title' });
  section.hidden = true;
  const heading = node(document, 'h3', 'People in your organization', { id: 'share-team-title' });
  const content = node(document, 'div', '', { class: 'share-team-content' });
  content.hidden = true;
  const intro = node(document, 'p', 'Choose who in your organization can open this app and what they can do. They see live updates.', { class: 'share-description' });
  const form = node(document, 'form', '', { class: 'share-create share-team-form' });
  const personLabel = node(document, 'label', 'Person');
  const person = node(document, 'select', '', { name: 'grantee', required: '' });
  personLabel.append(person);
  const roleLabel = node(document, 'label', 'Access');
  const role = node(document, 'select', '', { name: 'role' });
  roleLabel.append(role);
  const submit = node(document, 'button', 'Share', { type: 'submit', class: 'share-primary' });
  form.append(personLabel, roleLabel, submit);
  const privateNote = node(document, 'p', 'This app has private lists. There, people see and change only their own entries, unless they can review all entries.', { class: 'share-help share-private-note' });
  const everyone = node(document, 'div', '', { class: 'share-everyone' });
  const everyoneTitle = node(document, 'strong', 'Everyone in the organization');
  const everyoneState = node(document, 'p', '', { class: 'share-help' });
  const everyoneForm = node(document, 'div', '', { class: 'share-everyone-actions' });
  const everyoneLabel = node(document, 'label', 'Access for everyone');
  const everyoneRole = node(document, 'select', '', { name: 'everyone_role' });
  everyoneLabel.append(everyoneRole);
  const everyoneShare = node(document, 'button', 'Share with everyone', { type: 'button', class: 'share-secondary' });
  everyoneForm.append(everyoneLabel, everyoneShare);
  // Outside the form: a live share with everyone can always be stopped
  // (review NIT-2), even on a work now shared person by person only.
  const everyoneStop = node(document, 'button', 'Stop sharing with everyone', { type: 'button', class: 'share-danger share-stop' });
  const status = node(document, 'p', '', { role: 'status', 'aria-live': 'polite', class: 'share-status' });
  const cancelled = () => { status.textContent = 'Nothing was changed.'; };
  const personConfirm = confirmation(document, cancelled);
  const everyoneConfirm = confirmation(document, cancelled);
  everyone.append(everyoneTitle, everyoneState, everyoneForm, everyoneConfirm.box, everyoneStop);
  const listTitle = node(document, 'h4', 'Shared with', { class: 'share-team-list-title' });
  const list = node(document, 'div', '', { class: 'share-people share-team-list', 'aria-label': 'People in your organization this app is shared with' });
  const retry = node(document, 'button', 'Try again', { type: 'button', class: 'share-secondary share-retry' });
  retry.hidden = true;
  content.append(intro, form, personConfirm.box, privateNote, everyone, listTitle, list);
  section.append(heading, content, status, retry);

  let sharing: TeamSharing | null = null;
  let busy = false;
  let nobody = true;
  const controls = [person, role, everyoneRole, everyoneShare, everyoneStop];
  const freeze = (value: boolean) => { busy = value; for (const control of controls) control.disabled = value; submit.disabled = value || nobody; };
  const message = (error: unknown) => error instanceof Error ? error.message : 'Sharing could not be updated.';
  const roleOptions = (select: HTMLSelectElement, roles: TeamRole[], chosen: TeamRole) => {
    select.replaceChildren();
    for (const value of roles) {
      const option = node(document, 'option', ROLE_WORDS[value], { value });
      option.selected = value === chosen;
      select.append(option);
    }
  };
  function render(current: TeamSharing) {
    const roles = offeredRoles(current);
    roleOptions(role, roles, role.value && roles.includes(role.value as TeamRole) ? role.value as TeamRole : 'viewer');
    const live = current.organization.role;
    roleOptions(everyoneRole, roles, live && roles.includes(live) ? live : 'viewer');
    privateNote.hidden = !current.privateLists;
    everyoneForm.hidden = !current.organization.available;
    everyoneStop.hidden = !live;
    if (!current.organization.available) {
      everyoneState.textContent = live
        ? `Shared with everyone: ${ROLE_WORDS[live]}. ${UNAVAILABLE_FOR_EVERYONE}`
        : UNAVAILABLE_FOR_EVERYONE;
    } else {
      everyoneState.textContent = live
        ? `Shared with everyone: ${ROLE_WORDS[live]}. This includes people who join later.`
        : 'Not shared with everyone. Sharing with everyone includes people who join later.';
      everyoneShare.textContent = live ? 'Change access for everyone' : 'Share with everyone';
    }
    list.replaceChildren();
    if (!current.members.length) list.append(node(document, 'p', 'Not shared with anyone in your organization yet.', { class: 'share-empty' }));
    for (const member of current.members) {
      const row = node(document, 'section', '', { class: 'share-person share-member' });
      const detail = node(document, 'div');
      detail.append(node(document, 'strong', member.label), node(document, 'p', ROLE_WORDS[member.role], { class: 'share-help' }));
      row.append(detail);
      if (current.canRevokeMembers) {
        const actions = node(document, 'div', '', { class: 'share-person-actions' });
        const remove = node(document, 'button', 'Remove access', { type: 'button', class: 'share-danger', 'aria-label': `Remove access for ${member.label}` });
        remove.addEventListener('click', () => void act(async () => {
          await revokeMemberShare(app, work, member.principal_id);
          return `${member.label} no longer has access.`;
        }, remove));
        actions.append(remove); row.append(actions);
      }
      list.append(row);
    }
  }
  async function refresh(): Promise<TeamSharing | null> {
    const next = await readTeamSharing(app, work);
    if (!section.isConnected) return null;
    sharing = next;
    if (!next) { section.hidden = true; return null; }
    section.hidden = false; content.hidden = false; retry.hidden = true; render(next); return next;
  }
  async function act(change: () => Promise<string | ((after: TeamSharing | null) => string)>, button?: HTMLButtonElement) {
    if (busy) return;
    personConfirm.close(); everyoneConfirm.close();
    freeze(true); if (button) button.disabled = true;
    status.textContent = 'Updating access…';
    try {
      const done = await change();
      const after = await refresh();
      status.textContent = typeof done === 'function' ? done(after) : done;
    } catch (error) {
      status.textContent = message(error);
      try { await refresh(); } catch { /* the message above stands */ }
    } finally { freeze(false); }
  }
  function sharePerson(grantee: string, label: string, chosen: TeamRole) {
    // A lower role takes the surplus away (W4's demotion); the list then
    // shows the person's live access, and the message says what changed.
    const before = sharing?.members.find(member => member.principal_id === grantee)?.role;
    void act(async () => {
      await shareWithMember(app, work, grantee, chosen);
      return (after: TeamSharing | null) => {
        const held = after?.members.find(member => member.principal_id === grantee)?.role;
        if (held && RANK[held] > RANK[chosen]) return `${label} already had more access, so it stays “${ROLE_WORDS[held]}”.`;
        if (before && held === chosen && before !== chosen) return `${label} now has “${ROLE_WORDS[chosen]}”.`;
        return `Shared with ${label}: ${ROLE_WORDS[chosen]}.`;
      };
    }, submit);
  }
  function shareEveryone(chosen: TeamRole) {
    void act(async () => { await shareWithEveryone(app, work, chosen); return `Shared with everyone in the organization: ${ROLE_WORDS[chosen]}.`; }, everyoneShare);
  }
  form.addEventListener('submit', event => {
    event.preventDefault();
    if (busy || !sharing) return;
    const grantee = person.value;
    const chosen = role.value as TeamRole;
    const label = person.selectedOptions[0]?.textContent ?? 'This person';
    if (!grantee) { status.textContent = 'Choose a person first.'; return; }
    // Review MINOR-7: the reviewer role is named before it is given.
    if (chosen === 'reviewer') {
      personConfirm.ask(`${label} will see all entries, including private ones, and can take review actions.`, 'Yes, share', () => sharePerson(grantee, label, chosen));
      return;
    }
    sharePerson(grantee, label, chosen);
  });
  everyoneShare.addEventListener('click', () => {
    if (busy) return;
    const chosen = everyoneRole.value as TeamRole;
    if (chosen === 'reviewer') {
      everyoneConfirm.ask('Everyone in the organization will see all entries, including private ones, and can take review actions. This includes people who join later.', 'Let everyone review all entries', () => shareEveryone(chosen));
      return;
    }
    shareEveryone(chosen);
  });
  everyoneStop.addEventListener('click', () => {
    void act(async () => { await shareWithEveryone(app, work, 'none'); return 'Stopped sharing with everyone. People you added by name keep their access.'; }, everyoneStop);
  });
  async function load() {
    retry.hidden = true;
    try {
      if (!await refresh()) return;
      // The caller is never in /v1/members; the app's owner is left out too
      // (no share names them).
      const colleagues = (await readColleagues()).filter(colleague => colleague.principal_id !== sharing?.owner);
      if (!section.isConnected) return;
      person.replaceChildren(node(document, 'option', colleagues.length ? 'Choose a person' : 'No one else is in your organization yet', { value: '', disabled: '', selected: '' }));
      for (const colleague of colleagues) person.append(node(document, 'option', colleague.label, { value: colleague.principal_id }));
      nobody = !colleagues.length; submit.disabled = nobody;
    } catch (error) {
      // Review MINOR-3: an outage is said, with a retry, never a vanished section.
      if (!section.isConnected) return;
      section.hidden = false;
      content.hidden = sharing === null;
      status.textContent = error instanceof ShareError ? error.message : message(error);
      retry.hidden = false;
    }
  }
  retry.addEventListener('click', () => void load());
  return { section, load };
}

export function startSharing(document: Document): void {
  document.addEventListener('click', event => {
    const trigger = (event.target as Element | null)?.closest<HTMLElement>('[data-manage-share]');
    if (trigger) openSharing(document, trigger.dataset.app ?? '', trigger.dataset.work ?? '', trigger.dataset.title ?? 'App');
  });
}
