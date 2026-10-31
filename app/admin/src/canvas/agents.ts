// "Connect an agent" (APP-099): the member connects any MCP client (Claude
// Code, the OpenAI Responses API, Cursor) to Forma. The panel mints a
// labelled agent token through `POST /v1/agent-tokens` (same-origin, the
// canvas session), shows it once with a copy button, lists the member's
// connected agents (`GET /v1/agent-tokens`) and revokes one
// (`DELETE /v1/agent-tokens/{id}`). The setup snippets carry the endpoint and
// a placeholder only, never a live token. Nothing here grants anything: a
// token acts as this member, with this member's permissions, receipts and
// audit, and the server re-judges every call.
import { when as displayWhen } from './display';
import { activityButton, scopeFields } from './governance';
import { el, isUuid } from './render';

export const TOKENS_URL = '/v1/agent-tokens';
const PLACEHOLDER = '<YOUR_FORMA_TOKEN>';

type TokenJson = { token_id: string; label: string; created_at: string; last_used_at: string | null; expires_at?: string | null };

function mcpUrl(served: unknown): string {
  return typeof served === 'string' && /^https?:\/\/[^/]+\/v1\/mcp$/.test(served)
    ? served
    : `${window.location.origin}/v1/mcp`;
}

// The token never goes on a command line (argv, shell history): Claude Code
// expands `${FORMA_AGENT_TOKEN}` from the environment in a project `.mcp.json`.
function claudeSnippet(url: string): string {
  return JSON.stringify(
    { mcpServers: { forma: { type: 'http', url, headers: { Authorization: 'Bearer ${FORMA_AGENT_TOKEN}' } } } },
    null,
    2,
  );
}
function openaiSnippet(url: string): string {
  return JSON.stringify(
    { type: 'mcp', server_label: 'forma', server_url: url, headers: { Authorization: `Bearer ${PLACEHOLDER}` }, require_approval: 'never' },
    null,
    2,
  );
}

async function copy(text: string, button: HTMLButtonElement): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    button.textContent = 'Copied';
  } catch {
    button.textContent = 'Select and copy';
  }
  window.setTimeout(() => { button.textContent = 'Copy'; }, 1600);
}

function copyable(document_: Document, label: string, text: string, multiline = false): HTMLElement {
  const field = multiline
    ? el(document_, 'textarea', { class: 'cv-agent-code', readonly: '', rows: '7', 'aria-label': label, spellcheck: 'false' })
    : el(document_, 'input', { class: 'cv-agent-code', readonly: '', type: 'text', 'aria-label': label, spellcheck: 'false' });
  field.value = text;
  const button = el(document_, 'button', { type: 'button', class: 'cv-agent-copy' }, 'Copy');
  button.addEventListener('click', () => void copy(field.value, button));
  return el(document_, 'div', { class: 'cv-agent-copyable' }, el(document_, 'span', { class: 'cv-agent-copy-label' }, label), field, button);
}

/** A token's time as the rest of the canvas shows times: `Oct 22, 10:03 UTC` (`display.ts`). */
function when(stamp: string | null): string {
  if (!stamp) return 'never used';
  const date = new Date(stamp);
  if (Number.isNaN(date.getTime())) return stamp;
  const shown = displayWhen(date.toISOString());
  return shown ? `${shown} UTC` : stamp;
}

/** `expires in 29 days` from the server's `expires_at` (empty when it sent none). */
export function expiresIn(stamp: string | null | undefined, now = Date.now()): string {
  if (!stamp) return '';
  const at = new Date(stamp).getTime();
  if (Number.isNaN(at)) return '';
  const left = at - now;
  if (left <= 0) return 'expired';
  const days = Math.floor(left / 86_400_000);
  if (days === 0) return 'expires today';
  if (days === 1) return 'expires in 1 day';
  return `expires in ${days} days`;
}

async function listTokens(): Promise<{ tokens: TokenJson[]; url: string } | null> {
  try {
    const response = await fetch(TOKENS_URL, { credentials: 'same-origin', cache: 'no-store' });
    if (!response.ok) return null;
    const body = (await response.json()) as { tokens?: unknown; mcp_url?: unknown };
    const tokens = Array.isArray(body.tokens)
      ? (body.tokens as TokenJson[]).filter((t) => isUuid(t.token_id) && typeof t.label === 'string')
      : [];
    return { tokens, url: mcpUrl(body.mcp_url) };
  } catch {
    return null;
  }
}

function buildPanel(document_: Document): HTMLDialogElement {
  const status = el(document_, 'p', { class: 'cv-agent-status', role: 'status' });
  const minted = el(document_, 'div', { class: 'cv-agent-minted', hidden: '' });
  const list = el(document_, 'ul', { class: 'cv-agent-list', 'aria-label': 'Connected agents' });
  const snippets = el(document_, 'div', { class: 'cv-agent-snippets' });
  const input = el(document_, 'input', { type: 'text', name: 'label', maxlength: '40', value: 'Claude', 'aria-label': 'Agent name', required: '' });
  const create = el(document_, 'button', { type: 'submit', class: 'cv-agent-create' }, 'Create token');
  const scope = scopeFields(document_); // APP-115
  const form = el(document_, 'form', { class: 'cv-agent-form' },
    el(document_, 'label', { class: 'cv-agent-label' }, 'Name this agent', input), scope.element, create);
  const close = el(document_, 'button', { type: 'button', class: 'cv-agent-close', 'aria-label': 'Close' }, '×');
  const dialog = el(document_, 'dialog', { class: 'cv-agent-panel', 'aria-labelledby': 'cv-agent-title' },
    el(document_, 'header', { class: 'cv-agent-head' }, el(document_, 'h2', { id: 'cv-agent-title' }, 'Connect an agent'), activityButton(document_), close),
    el(document_, 'p', { class: 'cv-agent-copy-text' },
      'Any MCP client — Claude, OpenAI, Cursor or your own agent — acts through Forma as you: your permissions, receipts on every number, every write an audited command that asks you to confirm.'),
    form, status, minted,
    el(document_, 'h3', {}, 'Set it up'), snippets,
    el(document_, 'h3', {}, 'Connected agents'), list);

  const render = async (): Promise<void> => {
    const listed = await listTokens();
    if (!listed) {
      list.replaceChildren(el(document_, 'li', { class: 'cv-agent-empty' }, 'The connected agents could not be read right now.'));
      return;
    }
    snippets.replaceChildren(
      copyable(document_, 'Claude Code (.mcp.json in your project)', claudeSnippet(listed.url), true),
      copyable(document_, 'OpenAI Responses API (tools entry)', openaiSnippet(listed.url), true),
      el(document_, 'p', { class: 'cv-agent-note' }, `For Claude Code, export the token as FORMA_AGENT_TOKEN in your shell; it never goes on a command line. For OpenAI, replace ${PLACEHOLDER} in your own code. Forma keeps only a digest of the token.`),
    );
    if (!listed.tokens.length) {
      list.replaceChildren(el(document_, 'li', { class: 'cv-agent-empty' }, 'No agent is connected.'));
      return;
    }
    list.replaceChildren(...listed.tokens.map((token) => {
      const revoke = el(document_, 'button', { type: 'button', class: 'cv-agent-revoke' }, 'Revoke');
      revoke.addEventListener('click', () => void (async () => {
        revoke.disabled = true;
        let ok = false;
        try {
          const response = await fetch(`${TOKENS_URL}/${token.token_id}`, { method: 'DELETE', credentials: 'same-origin' });
          ok = response.status === 204;
        } catch { /* reported below */ }
        status.textContent = ok ? `${token.label} disconnected. Its token no longer works.` : 'The token could not be revoked right now.';
        await render();
      })());
      return el(document_, 'li', { class: 'cv-agent-item' },
        el(document_, 'span', { class: 'cv-agent-name' }, token.label),
        el(document_, 'span', { class: 'cv-agent-meta', title: token.expires_at ? `Expires ${when(token.expires_at)}` : '' },
          [`created ${when(token.created_at)}`, token.last_used_at ? `last used ${when(token.last_used_at)}` : 'never used', expiresIn(token.expires_at)].filter(Boolean).join(' · ')),
        revoke);
    }));
  };

  form.addEventListener('submit', (event) => {
    event.preventDefault();
    const label = input.value.trim();
    if (!/^[A-Za-z0-9][A-Za-z0-9 ._-]{0,39}$/.test(label)) {
      status.textContent = 'Use 1 to 40 letters, digits, spaces, dots, dashes or underscores.';
      return;
    }
    const chosen = scope.read();
    if (typeof chosen === 'string') {
      status.textContent = chosen;
      return;
    }
    create.disabled = true;
    void (async () => {
      try {
        const response = await fetch(TOKENS_URL, {
          method: 'POST', credentials: 'same-origin', cache: 'no-store',
          headers: { 'content-type': 'application/json' }, body: JSON.stringify({ label, ...chosen }),
        });
        const body = (await response.json().catch(() => null)) as { token?: unknown; label?: unknown; expires_at?: unknown } | null;
        if (response.status === 201 && body && typeof body.token === 'string') {
          minted.hidden = false;
          minted.replaceChildren(
            el(document_, 'p', { class: 'cv-agent-once' }, `Token for ${String(body.label)}. It is shown once: copy it into your agent now.${
              typeof body.expires_at === 'string' && expiresIn(body.expires_at) ? ` It ${expiresIn(body.expires_at)}.` : ''}`),
            copyable(document_, 'Token', body.token),
          );
          status.textContent = '';
        } else {
          status.textContent = response.status === 409
            ? 'You have the maximum number of connected agents. Revoke one first.'
            : response.status === 400 ? 'That scope or expiry is not allowed.'
            : response.status === 429 ? 'Too many tokens created recently. Try again later.' : 'The token could not be created right now.';
        }
      } catch {
        status.textContent = 'The token could not be created right now.';
      } finally {
        create.disabled = false;
        await render();
      }
    })();
  });
  close.addEventListener('click', () => {
    // The raw token leaves the page when the panel closes.
    minted.replaceChildren();
    minted.hidden = true;
    dialog.close();
  });
  dialog.addEventListener('close', () => { minted.replaceChildren(); minted.hidden = true; });
  dialog.addEventListener('cv-open', () => void render());
  return dialog;
}

/** Adds the "Connect an agent" button to the top bar and its panel to the page. */
export function startAgents(document_: Document): void {
  const bar = document_.querySelector<HTMLElement>('.cv-top-status');
  if (!bar || bar.querySelector('.cv-agent-open')) return;
  const panel = buildPanel(document_);
  document_.body.append(panel);
  const open = el(document_, 'button', { type: 'button', class: 'cv-agent-open', 'aria-haspopup': 'dialog' }, 'Connect an agent');
  open.addEventListener('click', () => {
    panel.dispatchEvent(new Event('cv-open'));
    if (typeof panel.showModal === 'function') panel.showModal();
    else panel.setAttribute('open', '');
  });
  const anchor = bar.querySelector('.cv-link');
  if (anchor) bar.insertBefore(open, anchor);
  else bar.prepend(open);
}
