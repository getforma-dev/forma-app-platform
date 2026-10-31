// APP-103: the shell every widget card shares: a header (mark, title, the
// "widget" label, remove), the widget's body, and a "shape in words" field
// (APP-122: the Restyle field renamed Shape; `/shape <widget>` reaches the same
// mechanism)
// that posts the member's sentence with the widget's closed state to
// `POST /v1/widgets/customize`. The answer is judged again here
// (`validState`) and applied only as data attributes the widgets stylesheet
// keys on; a refusal keeps the state. The sentence travels in a JSON body
// only, never in a URL.
import { el } from '../render';
import { showTiming } from '../timing';
import { validState, type WidgetKind, type WidgetState } from './state';

export const CUSTOMIZE_URL = '/v1/widgets/customize';

export type WidgetCard = {
  id: string;
  kind: WidgetKind;
  card: HTMLElement;
  body: HTMLElement;
  chips: HTMLElement;
  state: WidgetState;
  announce(message: string): void;
  /** Applies a state already judged by `validState`. */
  apply(state: WidgetState): void;
  onState(listener: (state: WidgetState, previous: WidgetState) => void): void;
  onRemove(listener: () => void): void;
};

const TITLES: Record<WidgetKind, [mark: string, eyebrow: string, title: string]> = {
  weather: ['WX', 'Environment', 'Weather'],
  calculator: ['±', 'Utility', 'Calculator'],
  clock: ['88', 'Time', 'Clock'],
};

const HINTS: Record<WidgetKind, string> = {
  weather: 'Try “show Celsius” or “make it glass”',
  calculator: 'Try “make it a TI-84 in Matrix style”',
  clock: 'Try “24 hour” or “make it paper”',
};

export function widgetTitle(kind: WidgetKind): string {
  return TITLES[kind][2];
}

function look(card: HTMLElement, state: WidgetState): void {
  card.dataset.skin = state.skin;
  card.dataset.accent = state.accent;
  card.dataset.font = state.font;
}

export function buildCard(document_: Document, kind: WidgetKind, id: string, state: WidgetState): WidgetCard {
  const [mark, eyebrow, title] = TITLES[kind];
  const status = el(document_, 'p', { class: 'wg-status', role: 'status' });
  const chips = el(document_, 'span', { class: 'wg-chips' }, el(document_, 'span', { class: 'badge wg-badge' }, 'widget'));
  const close = el(document_, 'button', { type: 'button', class: 'wg-close', 'aria-label': `Remove ${title.toLowerCase()}`, title: 'Remove' }, '×');
  const body = el(document_, 'div', { class: 'wg-body' });
  const input = el(document_, 'input', {
    type: 'text', name: 'shape', maxlength: '300', autocomplete: 'off', placeholder: HINTS[kind], 'aria-label': `Shape the ${title.toLowerCase()} in words`,
  });
  const submit = el(document_, 'button', { type: 'submit' }, 'Shape');
  const form = el(document_, 'form', { class: 'wg-customize', autocomplete: 'off' }, input, submit);
  const card = el(document_, 'article', {
    class: 'cv-card cv-widget', 'data-kind': 'widget', 'data-widget': kind, 'data-widget-id': id, 'aria-label': title,
  },
  el(document_, 'div', { class: 'wg-decor', 'aria-hidden': 'true' }),
  el(document_, 'header', { class: 'wg-head' },
    el(document_, 'span', { class: 'wg-mark', 'aria-hidden': 'true' }, mark),
    el(document_, 'div', { class: 'wg-id' }, el(document_, 'p', { class: 'wg-eyebrow' }, eyebrow), el(document_, 'h2', { class: 'wg-title' }, title)),
    chips,
    close),
  body,
  form,
  status);
  const stateListeners: Array<(state: WidgetState, previous: WidgetState) => void> = [];
  const removeListeners: Array<() => void> = [];
  const widget: WidgetCard = {
    id,
    kind,
    card,
    body,
    chips,
    state,
    announce(message) {
      status.textContent = message;
    },
    apply(next) {
      const previous = widget.state;
      widget.state = next;
      look(card, next);
      for (const listener of stateListeners) listener(next, previous);
    },
    onState(listener) {
      stateListeners.push(listener);
    },
    onRemove(listener) {
      removeListeners.push(listener);
    },
  };
  look(card, state);
  close.addEventListener('click', () => {
    for (const listener of removeListeners) listener();
  });
  form.addEventListener('submit', (event) => {
    event.preventDefault();
    const text = input.value.trim();
    if (!text || submit.disabled) return;
    submit.disabled = true;
    void customize(widget, text).finally(() => {
      submit.disabled = false;
    });
  });
  // Keys typed into the Shape field belong to the field, not the canvas.
  form.addEventListener('keydown', (event) => event.stopPropagation());
  return widget;
}

const REFUSALS: Record<string, string> = {
  could_not_customize: 'That shape is outside what this widget can show. Nothing changed.',
  composer_unavailable: 'Shaping in words is not available on this server right now. Nothing changed.',
  too_many_requests: 'Too many shapes in a short time. Try again in a moment.',
  invalid_request: 'Say it in a short sentence (up to 300 characters). Nothing changed.',
};

/**
 * Restyles `widget` from the member's sentence. The answer applies only
 * when it is the same closed state; anything else keeps the widget as it is.
 */
export async function customize(widget: WidgetCard, text: string): Promise<boolean> {
  widget.card.dataset.busy = 'true';
  widget.announce('Shaping…');
  const started = performance.now();
  try {
    let response: Response;
    try {
      response = await fetch(CUSTOMIZE_URL, {
        method: 'POST',
        credentials: 'same-origin',
        cache: 'no-store',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ widget: widget.kind, state: widget.state, text }),
      });
    } catch {
      widget.announce('The shape could not be sent. Nothing changed.');
      return false;
    }
    const body = (await response.json().catch(() => null)) as Record<string, unknown> | null;
    if (!response.ok) {
      const code = typeof body?.code === 'string' ? body.code : '';
      widget.announce(REFUSALS[code] ?? 'The shape could not be applied right now. Nothing changed.');
      return false;
    }
    const next = body && body.widget === widget.kind ? validState(widget.kind, body.state) : null;
    if (!next) {
      widget.announce(REFUSALS.could_not_customize ?? '');
      return false;
    }
    const ms = Math.round(performance.now() - started);
    widget.card.ownerDocument.getElementById('cv-canvas')?.setAttribute('data-last-restyle-ms', String(ms));
    widget.apply(next);
    // APP-116: the round trip, with the server's own duration (the model's turn included).
    const server = typeof body?.duration_ms === 'number' ? body.duration_ms : null;
    showTiming('Shape', [{ label: body?.source === 'model' ? 'shape (model)' : 'shape', ms, server }], widget.card);
    // ADR-011 I5: a state the model chose is labeled as such.
    if (body?.source === 'model' && !widget.chips.querySelector('.wg-ai')) {
      widget.chips.append(el(widget.card.ownerDocument, 'span', { class: 'badge wg-ai', title: 'Shaped by the model within the widget’s fixed styles' }, 'AI-styled'));
    }
    widget.announce(body?.changed === false ? 'Already looks like that.' : `Shaped in ${ms} ms.`);
    return true;
  } finally {
    delete widget.card.dataset.busy;
  }
}
