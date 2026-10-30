// APP-108: the Brief card of a folder ("AI brief · cited from your files").
// `GET /v1/boxes/{box}/brief` reads the last brief; `POST` (re)generates it
// (Regenerate, `/folder formation brief …`, or once automatically when a
// folder's "Brief" finds none). The server cites every statement: a text
// part may carry a cite (file, page) shown as a chip that opens the viewer at
// that page; a governed number is a value part naming its view and receipt,
// shown as a button that brings that view card forward with its receipt.
// Files the plan cannot include say so ("too large for the brief in this
// plan"). Text nodes only; nothing is stored in the browser.
import { brief, NOUN, refusalText, type Brief, type BriefFile, type BriefPart } from './box-api';
import { openViewer } from './box-viewer';
import { canvasApi, mountWidget, unmountWidget } from './board';
import { showValue } from './explain';
import { registerFocusCard, unregisterFocusCard } from './focus';
import { el } from './render';

const LABEL = 'AI brief · cited from your files';
const cards = new Map<string, { item: HTMLElement; render(found: Brief): void; say(message: string): void; regenerate(): Promise<void> }>();

function partNodes(document_: Document, boxId: string, part: BriefPart, files: BriefFile[]): Node[] {
  if ('value' in part) {
    const short = part.receipt_digest.replace(/^sha256:/, '').slice(0, 8);
    const button = el(document_, 'button', {
      type: 'button', class: 'cv-ex-value',
      title: `${part.exact ? `Exact ${part.exact}` : part.value} · ${part.view_title}${short ? ` · receipt ${short}` : ''}`,
    }, part.value);
    if (part.app_id && part.work_id) button.addEventListener('click', () => void showValue(part.app_id, part.work_id, part.view_id));
    else button.disabled = true;
    return [button];
  }
  if ('cite' in part) {
    const { cite } = part;
    const known = files.find((file) => file.file_id === cite.file_id);
    const chip = el(document_, 'button', { type: 'button', class: 'cv-br-cite', title: `Open ${cite.name}${cite.page ? ` at page ${cite.page}` : ''}` },
      cite.page ? `${cite.name}, p. ${cite.page}` : cite.name);
    chip.addEventListener('click', () => openViewer({
      file_id: cite.file_id, name: cite.name, kind: known?.kind ?? (cite.page ? 'pdf' : 'text'), media_type: '', bytes: 0, sha256: '',
      created_at: '', content_url: `/v1/boxes/${boxId}/files/${cite.file_id}/content`, data: null,
    }, boxId, cite.page));
    return [document_.createTextNode(part.text), document_.createTextNode(' '), chip];
  }
  return [document_.createTextNode(part.text)];
}

function message(error: unknown): string {
  const code = (error as { code?: string } | null)?.code;
  if (code === 'composer_unavailable') return 'Briefs are not available on this server right now.';
  return refusalText(error);
}

/** Opens (or refreshes) the Brief card of folder `boxId` beside its card; `generate` POSTs first. */
export async function openBrief(boxId: string, name: string, generate: boolean, beside?: HTMLElement | null): Promise<string> {
  let entry = cards.get(boxId);
  if (entry) {
    canvasApi()?.centerItem(entry.item);
    if (generate) await entry.regenerate();
    return `The brief of ${name} is on your canvas.`;
  }
  const document_ = document;
  const overview = el(document_, 'p', { class: 'cv-br-overview' });
  const filesList = el(document_, 'ul', { class: 'cv-br-files', 'aria-label': 'Each file' });
  const points = el(document_, 'ul', { class: 'cv-br-points', 'aria-label': 'Key points' });
  const pointsSection = el(document_, 'section', {}, el(document_, 'h3', { class: 'bx-h' }, 'Key points'), points);
  const status = el(document_, 'p', { class: 'cv-ex-status', role: 'status' });
  const when = el(document_, 'p', { class: 'cv-ex-when' });
  const regenerate = el(document_, 'button', { type: 'button', class: 'cv-ex-regen' }, 'Regenerate');
  const close = el(document_, 'button', { type: 'button', class: 'bx-close', 'data-widget-close': '', 'aria-label': 'Remove the brief from the canvas', title: 'Remove' }, '×');
  const label = el(document_, 'span', { class: 'cv-origin cv-ex-label', 'data-origin': 'generated' }, LABEL);
  const head = el(document_, 'header', { class: 'cv-ex-head' },
    el(document_, 'div', { class: 'cv-ex-id' }, el(document_, 'h2', {}, 'Brief'), el(document_, 'p', { class: 'cv-ex-of' }, `${NOUN} ${name}`)),
    label);
  const card = el(document_, 'article', { class: 'cv-card cv-ex cv-brief', 'data-kind': 'brief', 'data-card': `br-${boxId}`, 'aria-label': `Brief of ${NOUN.toLowerCase()} ${name}` },
    head,
    overview, filesList, pointsSection, status,
    el(document_, 'footer', { class: 'cv-ex-foot' }, when, regenerate));
  const state = beside ? canvasApi()?.getItemState(beside) : null;
  const at = state && beside ? { x: state.x + beside.offsetWidth + 32, y: state.y } : undefined;
  const item = mountWidget(card, { id: `brief-${boxId}`, title: 'Brief', width: 480, minHeight: 300, at, focus: !at });
  if (!item) return 'The brief could not be placed.';
  // APP-106 focus mode (F or the focus button), as for every card.
  registerFocusCard(card, head, `${NOUN} ${name}`);
  head.append(close);
  let files: BriefFile[] = [];
  const say = (text: string) => { status.textContent = text; };
  const render = (found: Brief) => {
    files = found.files;
    label.textContent = found.label || LABEL;
    overview.replaceChildren(...found.overview.flatMap((part) => partNodes(document_, boxId, part, files)));
    filesList.replaceChildren(...found.files.map((file) => el(document_, 'li', { class: 'cv-br-file', 'data-status': file.status },
      el(document_, 'p', { class: 'cv-br-file-name' }, file.name),
      file.status === 'too_large'
        ? el(document_, 'p', { class: 'cv-br-note' }, 'Too large for the brief in this plan.')
        : file.status === 'names_only' && !file.paragraph.length
          ? el(document_, 'p', { class: 'cv-br-note' }, 'Included by name only.')
          : el(document_, 'p', {}, ...file.paragraph.flatMap((part) => partNodes(document_, boxId, part, files))))));
    points.replaceChildren(...found.key_points.map((point) => el(document_, 'li', {}, ...point.flatMap((part) => partNodes(document_, boxId, part, files)))));
    pointsSection.hidden = found.key_points.length === 0;
    say(found.state === 'none' ? 'No brief. The model’s answer was not used, or none was available.' : '');
    when.textContent = found.generated_at ? `Generated ${found.generated_at.replace('T', ' ').replace(/:\d\dZ$/, ' UTC')}` : '';
  };
  const remove = () => {
    unregisterFocusCard(card);
    unmountWidget(item);
    cards.delete(boxId);
  };
  const run = async (generate_: boolean) => {
    regenerate.disabled = true;
    if (generate_) say('Reading your files and writing the brief…');
    try {
      render(await brief(boxId, generate_));
    } catch (error) {
      if ((error as { status?: number } | null)?.status === 404 && !generate_) {
        // No brief read yet: generate one.
        await run(true);
        return;
      }
      if ((error as { status?: number } | null)?.status === 404) remove();
      else say(message(error));
    } finally {
      regenerate.disabled = false;
    }
  };
  entry = { item, render, say, regenerate: () => run(true) };
  cards.set(boxId, entry);
  regenerate.addEventListener('click', () => void run(true));
  close.addEventListener('click', remove);
  if (generate) {
    await run(true);
  } else {
    await run(false);
    // Nothing generated yet for this folder: once, automatically.
    if (cards.has(boxId) && !overview.childNodes.length && !filesList.childNodes.length && status.textContent?.startsWith('No brief')) await run(true);
  }
  return `The brief of ${name} is on your canvas.`;
}
