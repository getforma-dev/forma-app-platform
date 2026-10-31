// APP-115 quick wins: agent access governance on the canvas.
//
// - `scopeFields`: the scope a member chooses when creating an agent token in
//   "Connect an agent": all my apps (the default, today's behavior) or
//   selected spaces; the allowed actions; the expiry (at most 30 days). The
//   server judges every value again (`POST /v1/agent-tokens`, 400
//   `malformed_scope`); a scope only ever narrows the member's own rights.
// - `activityButton`: opens the "Agents" panel: the member's agents
//   (`GET /v1/governance/agents`) with scope, status, last use, call and
//   refusal counts and Revoke (`DELETE /v1/agent-tokens/{id}`), and one
//   agent's timeline (`GET /v1/governance/agents/{id}/activity`), newest
//   first: tool, touched resource, outcome chip (allowed, or refused with its
//   reason and space) and duration. While the panel is open it re-reads every
//   few seconds, so a refusal appears live (the events stream carries work
//   ids only; a per-member stream is APP-115 full).
//
// Everything shown is text the server returned, set as text content.
import { when as displayWhen } from './display';
import { el, isUuid } from './render';

const AGENTS_URL = '/v1/governance/agents';
const TOKENS_URL = '/v1/agent-tokens';
const REFRESH_MS = 4000;

const SPACES: [string, string][] = [
  ['finance', 'Finance'], ['security', 'Security'], ['operations', 'Operations'], ['sales', 'Sales'],
  ['support', 'Support'], ['people', 'People'], ['utilities', 'Widgets'], ['mine', 'My apps'], ['folders', 'Folders'],
];
const ACTIONS: [string, string][] = [
  ['read', 'Read'], ['draft', 'Draft'], ['commit', 'Commit'], ['decide', 'Decide'], ['share', 'Share'], ['build', 'Build apps'],
];
const EXPIRY_DAYS = [1, 7, 14, 30];

export type ScopeChoice = { scope?: { spaces: string[] }; actions?: string[]; expires_in_days: number };

type ScopeJson = { kind: string; spaces: string[]; apps: string[]; actions: string[]; space_titles?: string[]; app_titles?: string[] };
type AgentJson = {
  token_id: string; label: string; scope: ScopeJson; created_at: string; expires_at: string;
  last_used_at: string | null; status: string; calls: number; refused: number; last_call_at: string | null;
};
type EntryJson = {
  entry_id: string; sequence: number; tool: string; tool_title: string; outcome: string;
  reason: string | null; reason_title: string | null; space: string | null; space_title: string | null;
  resource_label: string | null; duration_ms: number; occurred_at: string;
  touched: { apps: string[]; works: string[]; records: string[]; folders: string[]; files: string[] };
};

function stamp(text: string | null): string {
  if (!text) return 'never';
  const date = new Date(text);
  if (Number.isNaN(date.getTime())) return text;
  const shown = displayWhen(date.toISOString());
  return shown ? `${shown} UTC` : text;
}

function checkbox(document_: Document, name: string, value: string, label: string, checked: boolean): HTMLLabelElement {
  const input = el(document_, 'input', { type: 'checkbox', name, value });
  input.checked = checked;
  return el(document_, 'label', { class: 'cv-gov-check' }, input, label);
}

/** The scope fieldset of the "Connect an agent" form and its reader. */
export function scopeFields(document_: Document): { element: HTMLElement; read: () => ScopeChoice | string } {
  const all = el(document_, 'input', { type: 'radio', name: 'cv-gov-scope', value: 'all' });
  all.checked = true;
  const selected = el(document_, 'input', { type: 'radio', name: 'cv-gov-scope', value: 'selected' });
  const spaces = el(document_, 'div', { class: 'cv-gov-options', role: 'group', 'aria-label': 'Spaces this agent may use' },
    ...SPACES.map(([id, title]) => checkbox(document_, 'cv-gov-space', id, title, false)));
  spaces.hidden = true;
  const sync = (): void => { spaces.hidden = !selected.checked; };
  all.addEventListener('change', sync);
  selected.addEventListener('change', sync);
  const actions = el(document_, 'div', { class: 'cv-gov-options', role: 'group', 'aria-label': 'Actions this agent may take' },
    ...ACTIONS.map(([id, title]) => checkbox(document_, 'cv-gov-action', id, title, true)));
  const expiry = el(document_, 'select', { name: 'cv-gov-expiry', 'aria-label': 'Token expires after' },
    ...EXPIRY_DAYS.map((days) => el(document_, 'option', { value: String(days) }, days === 1 ? '1 day' : `${days} days`)));
  expiry.value = '30';
  const element = el(document_, 'fieldset', { class: 'cv-gov-scope' },
    el(document_, 'legend', {}, 'What this agent may touch'),
    el(document_, 'div', { class: 'cv-gov-options' },
      el(document_, 'label', { class: 'cv-gov-check' }, all, 'All my apps'),
      el(document_, 'label', { class: 'cv-gov-check' }, selected, 'Only selected spaces')),
    spaces,
    el(document_, 'p', { class: 'cv-gov-legend' }, 'Allowed actions'),
    actions,
    el(document_, 'label', { class: 'cv-gov-expiry' }, 'Expires after ', expiry),
    el(document_, 'p', { class: 'cv-agent-note' }, 'A scope only narrows your own rights: the agent never does more than you can.'));
  const read = (): ScopeChoice | string => {
    const ticked = (name: string): string[] =>
      [...element.querySelectorAll<HTMLInputElement>(`input[name="${name}"]`)].filter((i) => i.checked).map((i) => i.value);
    const chosenActions = ticked('cv-gov-action');
    if (!chosenActions.length) return 'Allow at least one action.';
    const days = Number(expiry.value);
    const choice: ScopeChoice = { expires_in_days: EXPIRY_DAYS.includes(days) ? days : 30 };
    if (chosenActions.length !== ACTIONS.length) choice.actions = chosenActions;
    if (selected.checked) {
      const chosenSpaces = ticked('cv-gov-space');
      if (!chosenSpaces.length) return 'Choose at least one space, or allow all your apps.';
      choice.scope = { spaces: chosenSpaces };
    }
    return choice;
  };
  return { element, read };
}

function scopeText(scope: ScopeJson): string {
  const where = scope.kind === 'all'
    ? 'All my apps'
    : [...(scope.space_titles ?? scope.spaces), ...(scope.app_titles ?? scope.apps)].join(', ');
  const what = scope.actions.length === ACTIONS.length ? 'every action' : scope.actions.join(', ');
  return `${where} · ${what}`;
}

function outcomeChip(document_: Document, entry: EntryJson): HTMLElement {
  if (entry.outcome === 'allowed') return el(document_, 'span', { class: 'cv-gov-chip', 'data-outcome': 'allowed' }, 'allowed');
  const space = entry.reason === 'scope_space' && entry.space_title ? ` (${entry.space_title})` : '';
  return el(document_, 'span', { class: 'cv-gov-chip', 'data-outcome': 'refused', title: entry.reason ?? '' },
    `refused: ${entry.reason_title ?? 'refused'}${space}`);
}

async function getJson<T>(url: string): Promise<T | null> {
  try {
    const response = await fetch(url, { credentials: 'same-origin', cache: 'no-store' });
    return response.ok ? ((await response.json()) as T) : null;
  } catch {
    return null;
  }
}

function buildPanel(document_: Document): HTMLDialogElement {
  const status = el(document_, 'p', { class: 'cv-agent-status', role: 'status' });
  const agents = el(document_, 'ul', { class: 'cv-gov-agents', 'aria-label': 'Your agents' });
  const timelineTitle = el(document_, 'h3', {}, 'Activity');
  const timeline = el(document_, 'ol', { class: 'cv-gov-timeline', 'aria-label': 'Agent activity, newest first', 'aria-live': 'polite' });
  const close = el(document_, 'button', { type: 'button', class: 'cv-agent-close', 'aria-label': 'Close' }, '×');
  const dialog = el(document_, 'dialog', { class: 'cv-agent-panel cv-gov-panel', 'aria-labelledby': 'cv-gov-title' },
    el(document_, 'header', { class: 'cv-agent-head' }, el(document_, 'h2', { id: 'cv-gov-title' }, 'Agents'), close),
    el(document_, 'p', { class: 'cv-agent-copy-text' },
      'Every call your agents make through Forma: what they touched, what was allowed and what was refused, and why.'),
    status, agents, timelineTitle, timeline);
  let selected: string | null = null;
  let seen = 0;
  let timer: number | undefined;

  const renderTimeline = async (): Promise<void> => {
    if (!selected) {
      timeline.replaceChildren(el(document_, 'li', { class: 'cv-agent-empty' }, 'Choose an agent to see its activity.'));
      return;
    }
    const body = await getJson<{ agent?: AgentJson; entries?: EntryJson[] }>(`${AGENTS_URL}/${selected}/activity`);
    if (!body || !Array.isArray(body.entries)) {
      timeline.replaceChildren(el(document_, 'li', { class: 'cv-agent-empty' }, 'The activity could not be read right now.'));
      return;
    }
    timelineTitle.textContent = body.agent ? `Activity of ${body.agent.label}` : 'Activity';
    if (!body.entries.length) {
      timeline.replaceChildren(el(document_, 'li', { class: 'cv-agent-empty' }, 'No calls yet.'));
      return;
    }
    const newest = body.entries[0]?.sequence ?? 0;
    timeline.replaceChildren(...body.entries.map((entry) => {
      const resource = [entry.resource_label, entry.space_title && entry.outcome === 'allowed' ? entry.space_title : null]
        .filter(Boolean).join(' · ');
      const ids = entry.touched.apps.length + entry.touched.works.length + entry.touched.records.length + entry.touched.folders.length + entry.touched.files.length;
      const row = el(document_, 'li', { class: 'cv-gov-entry', 'data-outcome': entry.outcome },
        el(document_, 'span', { class: 'cv-gov-when' }, stamp(entry.occurred_at)),
        el(document_, 'span', { class: 'cv-gov-tool' }, entry.tool_title),
        el(document_, 'span', { class: 'cv-gov-resource', title: ids ? `${ids} id(s) touched` : '' }, resource || '—'),
        outcomeChip(document_, entry),
        el(document_, 'span', { class: 'cv-gov-duration' }, `${entry.duration_ms} ms`));
      if (seen && entry.sequence > seen) row.classList.add('cv-gov-new');
      return row;
    }));
    seen = Math.max(seen, newest);
  };

  const render = async (): Promise<void> => {
    const body = await getJson<{ agents?: AgentJson[] }>(AGENTS_URL);
    if (!body || !Array.isArray(body.agents)) {
      agents.replaceChildren(el(document_, 'li', { class: 'cv-agent-empty' }, 'Your agents could not be read right now.'));
      return;
    }
    const list = body.agents.filter((a) => isUuid(a.token_id) && typeof a.label === 'string');
    if (!list.length) {
      agents.replaceChildren(el(document_, 'li', { class: 'cv-agent-empty' }, 'No agent is connected. Create one in "Connect an agent".'));
      selected = null;
      await renderTimeline();
      return;
    }
    if (!selected || !list.some((a) => a.token_id === selected)) selected = (list.find((a) => a.refused > 0) ?? list[0]).token_id;
    agents.replaceChildren(...list.map((agent) => {
      const show = el(document_, 'button', { type: 'button', class: 'cv-agent-copy', 'aria-pressed': String(agent.token_id === selected) }, 'Activity');
      show.addEventListener('click', () => { selected = agent.token_id; seen = 0; void render(); });
      const revoke = el(document_, 'button', { type: 'button', class: 'cv-agent-revoke' }, 'Revoke');
      revoke.hidden = agent.status !== 'live';
      revoke.addEventListener('click', () => void (async () => {
        revoke.disabled = true;
        let ok = false;
        try {
          const response = await fetch(`${TOKENS_URL}/${agent.token_id}`, { method: 'DELETE', credentials: 'same-origin' });
          ok = response.status === 204;
        } catch { /* reported below */ }
        status.textContent = ok ? `${agent.label} revoked. Its next call is refused.` : 'The token could not be revoked right now.';
        await render();
      })());
      return el(document_, 'li', { class: 'cv-gov-agent', 'data-selected': String(agent.token_id === selected) },
        el(document_, 'div', { class: 'cv-gov-agent-main' },
          el(document_, 'span', { class: 'cv-agent-name' }, agent.label),
          el(document_, 'span', { class: 'cv-gov-status', 'data-status': agent.status }, agent.status),
          el(document_, 'span', { class: 'cv-agent-meta' }, scopeText(agent.scope)),
          el(document_, 'span', { class: 'cv-agent-meta' },
            `${agent.calls} call${agent.calls === 1 ? '' : 's'} · ${agent.refused} refused · last used ${stamp(agent.last_call_at ?? agent.last_used_at)} · expires ${stamp(agent.expires_at)}`)),
        el(document_, 'div', { class: 'cv-gov-agent-actions' }, show, revoke));
    }));
    await renderTimeline();
  };

  const stop = (): void => { if (timer !== undefined) window.clearInterval(timer); timer = undefined; };
  close.addEventListener('click', () => dialog.close());
  dialog.addEventListener('close', stop);
  dialog.addEventListener('cv-open', () => {
    seen = 0;
    void render();
    stop();
    timer = window.setInterval(() => { if (dialog.open) void render(); else stop(); }, REFRESH_MS);
  });
  return dialog;
}

let panel: HTMLDialogElement | null = null;

/** Opens the "Agents" panel (built once per page). */
export function openGovernance(document_: Document): void {
  if (!panel || !panel.isConnected) {
    panel = buildPanel(document_);
    document_.body.append(panel);
  }
  panel.dispatchEvent(new Event('cv-open'));
  if (typeof panel.showModal === 'function') {
    if (!panel.open) panel.showModal();
  } else panel.setAttribute('open', '');
}

/** The "Activity" button of the "Connect an agent" panel. */
export function activityButton(document_: Document): HTMLButtonElement {
  const button = el(document_, 'button', { type: 'button', class: 'cv-gov-open', 'aria-haspopup': 'dialog' }, 'Activity');
  button.addEventListener('click', () => openGovernance(document_));
  return button;
}
