// Uploaded files are Cards. Questions stay scoped to the exact authorized file;
// the original stays available independently of any app made from it.
import { call, folderAnswer, refusalText, type BoxFile } from './box-api';
import { openViewer } from './box-viewer';
import { el } from './render';

export function fileCardFooter(document_: Document, boxId: string, file: BoxFile, makeApp?: () => void): HTMLElement {
  const input = el(document_, 'input', { type: 'text', maxlength: '500', placeholder: 'Ask anything about this file…', 'aria-label': `Ask about ${file.name}` });
  const ask = el(document_, 'button', { type: 'submit', class: 'cv-secondary' }, 'Ask');
  const answer = el(document_, 'div', { class: 'bx-file-answer', 'aria-live': 'polite', hidden: '' });
  const form = el(document_, 'form', { class: 'bx-file-ask', 'aria-label': `Ask AI about ${file.name}` }, input, ask);
  const actions = el(document_, 'div', { class: 'bx-file-actions' });
  if (makeApp) {
    const build = el(document_, 'button', { type: 'button', class: 'cv-secondary bx-file-build' }, 'Make an app');
    build.addEventListener('click', makeApp);
    actions.append(build);
  }
  const footer = el(document_, 'footer', { class: 'bx-file-footer' }, form, answer, actions);
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
