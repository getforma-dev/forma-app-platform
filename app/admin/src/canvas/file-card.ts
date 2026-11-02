// Uploaded files are Cards. Questions stay scoped to the exact authorized file;
// the original stays available independently of any app made from it.
import { call, folderAnswer, refusalText, type BoxFile } from './box-api';
import { openViewer } from './box-viewer';
import { el } from './render';

/** Fixed UI geometry only: filenames and answers remain text nodes. */
export function fileCardGlyph(document_: Document, kind: 'sheet' | 'open' | 'ask' | 'build' | 'close'): SVGSVGElement {
  const svg = document_.createElementNS('http://www.w3.org/2000/svg', 'svg');
  for (const [name, value] of Object.entries({ viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', 'stroke-width': '1.5', 'stroke-linecap': 'round', 'stroke-linejoin': 'round', 'aria-hidden': 'true', focusable: 'false' })) svg.setAttribute(name, value);
  const paths = {
    sheet: ['M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z', 'M14 2v6h6', 'M8 12h8v6H8Z', 'M8 15h8M12 12v6'],
    open: ['M13 4h7v7M20 4l-9 9', 'M10 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4'],
    ask: ['M12 19V5M6 11l6-6 6 6'],
    build: ['M4 4h6v6H4ZM14 4h6v6h-6ZM4 14h6v6H4ZM17 14v6M14 17h6'],
    close: ['m6 6 12 12M18 6 6 18'],
  }[kind];
  for (const d of paths) {
    const path = document_.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', d); svg.append(path);
  }
  return svg;
}

export function fileCardFooter(document_: Document, boxId: string, file: BoxFile, makeApp?: () => void, openFile?: () => void): HTMLElement {
  const input = el(document_, 'input', { type: 'text', maxlength: '500', placeholder: 'Ask anything about this file…', 'aria-label': `Ask about ${file.name}` });
  const ask = el(document_, 'button', { type: 'submit', class: 'bx-file-ask-submit', title: 'Ask about this file', 'aria-label': 'Ask' },
    fileCardGlyph(document_, 'ask'), el(document_, 'span', { class: 'sr-only' }, 'Ask'));
  const answer = el(document_, 'div', { class: 'bx-file-answer', 'aria-live': 'polite', hidden: '' });
  const form = el(document_, 'form', { class: 'bx-file-ask', 'aria-label': `Ask AI about ${file.name}` }, input, ask);
  const actions = el(document_, 'div', { class: 'bx-file-actions' });
  if (openFile) {
    const open = el(document_, 'button', { type: 'button', class: 'cv-secondary bx-file-open' }, fileCardGlyph(document_, 'open'), 'Open spreadsheet');
    open.addEventListener('click', openFile);
    actions.append(open);
  }
  if (makeApp) {
    const build = el(document_, 'button', { type: 'button', class: 'cv-secondary bx-file-build' }, fileCardGlyph(document_, 'build'), 'Make an app');
    build.addEventListener('click', makeApp);
    actions.append(build);
  }
  const footer = el(document_, 'footer', { class: `bx-file-footer${openFile ? '' : ' is-inline'}` }, actions, form, answer);
  let pending = false;
  form.addEventListener('submit', async event => {
    event.preventDefault();
    const question = input.value.trim();
    if (!question || pending) return;
    pending = true; ask.disabled = true;
    answer.hidden = false;
    answer.replaceChildren(el(document_, 'p', { role: 'status' }, 'Reading this file…'));
    try {
      const result = folderAnswer(await call('POST', `/v1/boxes/${boxId}/ask`, { question, file_id: file.file_id }));
      if (!result || result.references.some(ref => ref.file_id !== file.file_id)) throw new Error('invalid_file_answer');
      if (!footer.isConnected) return;
      answer.replaceChildren(el(document_, 'p', {}, result.text));
      if (result.references.length) {
        const sources = el(document_, 'details', {}, el(document_, 'summary', {}, `Source rows (${result.references.length})`));
        const rows = result.references.map(ref => {
          const link = el(document_, 'button', { type: 'button', class: 'bx-lineage-open', title: ref.sha256 },
            `${ref.file}${ref.sheet ? ` · ${ref.sheet}` : ''} · row ${ref.row}`);
          link.addEventListener('click', () => openViewer(file, boxId));
          return el(document_, 'li', {}, link);
        });
        sources.append(el(document_, 'ul', {}, ...rows)); answer.append(sources);
      }
    } catch (error) {
      if (footer.isConnected) answer.replaceChildren(el(document_, 'p', { role: 'status' }, refusalText(error)));
    } finally { pending = false; ask.disabled = false; }
  });
  return footer;
}
