// Original files belong to the app's Sources companion. Reopening the panel
// checks current authority; source names and contents are never kept in a cache.
import { contentUrl, type BoxFile, type FileKind } from './box-api';
import { openViewer } from './box-viewer';
import { fileCardGlyph } from './file-card';
import { isUuid } from './ids';
import { el } from './render';

export type AppSourceContext = {
  boxId: string;
  file: BoxFile;
  sourceId: string;
  revisionId: string;
};
export type AppSourcesOptions = {
  app: string;
  work: string;
  title: string;
  onAskFile(source: AppSourceContext): void;
};
export type AppSourcesController = { show(): void; hide(): void; dispose(): void; openFile(fileId: string): void };

const kindNames: Record<FileKind, string> = { csv: 'CSV', xlsx: 'Excel workbook', pdf: 'PDF', image: 'Image', text: 'Text' };

function sourcesOf(value: unknown): AppSourceContext[] | null {
  if (!value || typeof value !== 'object') return null;
  const result = value as Record<string, unknown>;
  if (!isUuid(result.box_id) || !Array.isArray(result.files)) return null;
  const seen = new Set<string>();
  const files: AppSourceContext[] = [];
  for (const entry of result.files) {
    if (!entry || typeof entry !== 'object') return null;
    const file = entry as Record<string, unknown>;
    if (!isUuid(file.file_id) || seen.has(file.file_id) || typeof file.name !== 'string'
      || !file.name || typeof file.kind !== 'string' || !Object.hasOwn(kindNames, file.kind)) return null;
    if (file.source_id != null && !isUuid(file.source_id)) return null;
    if (file.revision_id != null && !isUuid(file.revision_id)) return null;
    seen.add(file.file_id);
    files.push({
      boxId: result.box_id,
      sourceId: typeof file.source_id === 'string' ? file.source_id : '',
      revisionId: typeof file.revision_id === 'string' ? file.revision_id : '',
      // Lineage does not supply size/digest. The viewer omits absent metadata;
      // the original route authorizes each read independently.
      file: {
        file_id: file.file_id, name: file.name, kind: file.kind as FileKind,
        media_type: '', bytes: 0, sha256: '', created_at: '', data: null,
        content_url: contentUrl(result.box_id, file.file_id),
      },
    });
  }
  return files;
}

export function mountAppSources(host: HTMLElement, options: AppSourcesOptions): AppSourcesController {
  const document_ = host.ownerDocument;
  let requestedFile: string | null = null;
  let visible = false;
  let disposed = false;
  let generation = 0;
  let request: AbortController | null = null;
  const current = (ticket: number) => visible && !disposed && ticket === generation && host.isConnected;
  const stop = () => { generation++; request?.abort(); request = null; host.replaceChildren(); };

  const viewFile = (source: AppSourceContext, ticket: number) => {
    if (!current(ticket)) return;
    openViewer(source.file, source.boxId);
  };

  const load = async () => {
    stop();
    const ticket = generation;
    const selectedFile = requestedFile;
    requestedFile = null;
    if (!isUuid(options.app) || !isUuid(options.work)) return;
    const controller = new AbortController(); request = controller;
    host.append(el(document_, 'p', { class: 'cv-app-sources-status', role: 'status' }, 'Loading source files…'));
    try {
      const response = await fetch(`/v1/boxes/lineage/${options.app}/${options.work}`, {
        credentials: 'same-origin', cache: 'no-store', signal: controller.signal,
      });
      if (!current(ticket)) return;
      if ([401, 403, 404].includes(response.status)) {
        host.replaceChildren(el(document_, 'p', { class: 'cv-app-sources-status', role: 'status' }, 'No source files are available for this app.'));
        return;
      }
      if (!response.ok) throw new Error('unavailable');
      const sources = sourcesOf(await response.json());
      if (!current(ticket)) return;
      if (!sources) throw new Error('malformed_sources');
      if (!sources.length) {
        host.replaceChildren(el(document_, 'p', { class: 'cv-app-sources-status', role: 'status' }, 'This app has no source files.'));
        return;
      }
      const list = el(document_, 'ul', { class: 'cv-app-sources-list', 'aria-label': `Source files for ${options.title}` });
      for (const source of sources) {
        const view = el(document_, 'button', { type: 'button', class: 'cv-secondary cv-app-source-open' }, 'View original');
        view.addEventListener('click', () => viewFile(source, ticket));
        const ask = el(document_, 'button', { type: 'button', class: 'cv-secondary cv-app-source-ask' }, 'Ask about file');
        ask.addEventListener('click', () => { if (current(ticket)) options.onAskFile(source); });
        list.append(el(document_, 'li', { class: 'cv-app-source', 'data-app-source-file': source.file.file_id },
          el(document_, 'div', { class: 'cv-app-source-identity' },
            el(document_, 'span', { class: 'cv-app-source-glyph', 'aria-hidden': 'true' }, fileCardGlyph(document_, 'sheet')),
            el(document_, 'div', {}, el(document_, 'h4', { class: 'cv-app-source-name' }, source.file.name),
              el(document_, 'p', { class: 'cv-app-source-kind' }, `${kindNames[source.file.kind]} · Original file`))),
          el(document_, 'div', { class: 'cv-app-source-actions' }, view, ask)));
      }
      host.replaceChildren(el(document_, 'p', { class: 'cv-app-sources-intro' },
        sources.length === 1 ? 'The original file behind this app.' : `${sources.length} original files behind this app.`), list);
      const selected = selectedFile && sources.find(source => source.file.file_id === selectedFile);
      if (selected) viewFile(selected, ticket);
    } catch {
      if (!current(ticket)) return;
      const retry = el(document_, 'button', { type: 'button', class: 'cv-secondary' }, 'Try again');
      retry.addEventListener('click', () => { if (current(ticket)) void load(); });
      host.replaceChildren(el(document_, 'p', { class: 'cv-app-sources-status', role: 'status' }, 'Source files could not be loaded.'), retry);
    } finally { if (request === controller) request = null; }
  };
  return {
    show() { if (disposed || visible) return; visible = true; void load(); },
    openFile(fileId) { if (disposed || !isUuid(fileId)) return; requestedFile = fileId; if (visible) void load(); },
    hide() { visible = false; requestedFile = null; stop(); },
    dispose() { disposed = true; visible = false; stop(); },
  };
}
