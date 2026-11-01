import { requestAsk, type AskScope, type AskReply } from './client';
function el<K extends keyof HTMLElementTagNameMap>(doc: Document, tag: K, text = '', className = ''): HTMLElementTagNameMap[K] {
  const node = doc.createElement(tag); node.textContent = text; if (className) node.className = className; return node;
}
export function scopeLabel(scope: AskScope): string {
  if (scope.kind === 'widget') return `${scope.widget} widget · controls and settings`;
  if (scope.kind === 'canvas') return 'Canvas';
  return scope.address || (scope.kind === 'app' ? 'This app' : 'This part');
}
export function renderAskReply(doc: Document, answer: AskReply): HTMLElement {
  const section = el(doc, 'section', '', 'context-ask-answer'); section.append(el(doc, 'p', answer.text));
  if (answer.receipts.length) {
    const details = el(doc, 'details'); details.append(el(doc, 'summary', `Receipts (${answer.receipts.length})`));
    const list = el(doc, 'ol');
    for (const ref of answer.references) {
      const item = el(doc, 'li', ref.label); const receipt = answer.receipts.find(r => r.reference === ref.ref);
      item.append(el(doc, 'pre', JSON.stringify(receipt, null, 2))); list.append(item);
    }
    details.append(list); section.append(details);
  }
  return section;
}
export type AskPanel = { setScope(scope: AskScope, refusal?: string): void; focus(): void; invalidate(): void; dispose(): void };
/** Scope changes preserve the unsent question, abort the old request and clear its answer. */
export function mountAskPanel(host: HTMLElement, initial: AskScope, refusal = ''): AskPanel {
  const doc = host.ownerDocument;
  let scope = initial, unavailable = refusal, generation = 0, pending: AbortController | null = null;
  const label = el(doc, 'p', scopeLabel(scope), 'context-ask-label'); label.setAttribute('role', 'status');
  const form = el(doc, 'form', '', 'context-ask-form'); const input = el(doc, 'input'); input.type = 'text'; input.maxLength = 500;
  input.setAttribute('aria-label', 'Ask about this context'); input.placeholder = 'Ask a question about this'; input.autocomplete = 'off';
  const button = el(doc, 'button', 'Ask'); button.type = 'submit'; const output = el(doc, 'div', '', 'context-ask-output'); output.setAttribute('aria-live', 'polite');
  form.append(input, button); host.replaceChildren(label, form, output); host.classList.add('context-ask');
  const submit = async (event: Event) => {
    event.preventDefault(); if (pending || !input.value.trim()) return;
    const question = input.value.trim(); const at = ++generation; const current = scope;
    if (unavailable) { output.textContent = unavailable; return; }
    pending = new AbortController(); button.disabled = true; output.textContent = 'Reading…';
    const answer = await requestAsk(question, current, pending.signal);
    if (generation !== at) return;
    pending = null; button.disabled = false;
    if (typeof answer === 'string') output.textContent = answer;
    else { output.replaceChildren(renderAskReply(doc, answer)); if (input.value.trim() === question) input.value = ''; }
  };
  form.addEventListener('submit', submit); form.addEventListener('keydown', event => event.stopPropagation());
  const invalidate = () => { pending?.abort(); pending = null; generation += 1; button.disabled = false; output.replaceChildren(); };
  return {
    setScope(next, reason = '') { invalidate(); scope = next; unavailable = reason; label.textContent = scopeLabel(next); },
    focus() { input.focus(); },
    invalidate,
    dispose() { generation += 1; pending?.abort(); form.removeEventListener('submit', submit); host.replaceChildren(); },
  };
}
