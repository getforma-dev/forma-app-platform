// APP-108 (ADR-011 pitch): the boxes routes as typed client calls. Every
// answer is judged here before any module uses it: ids must be UUIDs, kinds
// and types closed sets, strings strings; anything else is dropped. Bodies are
// JSON (or one multipart part for an upload); data never travels in a URL
// beyond the ids. Refusals come back as the server's neutral code, shown
// through `refusalText`.
import { amount } from './display';
import { isUuid } from './render';

/**
 * The member-facing noun (Victor, 2026-10-30): "Folder" everywhere on the
 * canvas; code, ids and API paths keep "box". Change the word here only.
 */
export const NOUN = 'Folder';
export const noun = NOUN.toLowerCase();
export const NOUN_PLURAL = `${NOUN}s`;
export const nouns = NOUN_PLURAL.toLowerCase();
/** `/folder` and its aliases: adding a word is one entry here. */
export const FOLDER_COMMAND = noun;
export const FOLDER_ALIASES = ['storage', 'box', 'boxes', 'folders'];

/** An instruction that only asks for a brief (no build words). */
const BRIEF_WORDS = /\b(brief|explain|summari[sz]e|summary|what'?s in|what is in|overview)\b/i;
const BUILD_WORDS = /\b(build|app|apps|make|create|generate|dashboard|tracker|chart)\b/i;
export function wantsBrief(instruction: string): boolean {
  return BRIEF_WORDS.test(instruction) && !BUILD_WORDS.test(instruction);
}

/** The prefilled Formation prompt on a folder card. */
export const FORMATION_PROMPT = 'Build an app that shows the key metrics and explain them';

export const MAX_FILE_BYTES = 20 * 1024 * 1024;
export const MAX_BOX_FILES = 20;
export const ACCEPT = '.pdf,.png,.jpg,.jpeg,.webp,.csv,.xlsx,.txt,.md';
export const COLUMN_TYPES = ['text', 'integer', 'decimal', 'money', 'date', 'timestamp', 'boolean'] as const;
export type ColumnType = (typeof COLUMN_TYPES)[number];
export type FileKind = 'pdf' | 'image' | 'csv' | 'xlsx' | 'text';
const FILE_KINDS: FileKind[] = ['pdf', 'image', 'csv', 'xlsx', 'text'];

export type Box = {
  box_id: string; number: number; name: string; kind: 'box' | 'uploads' | 'app';
  app_id: string | null; owner: boolean; file_count: number; created_at: string;
};
export type DataInfo = { source_id: string; revision_id: string; normalized_digest: string; row_count: number; fields: { id: string }[] };
export type BoxFile = {
  file_id: string; name: string; kind: FileKind; media_type: string; bytes: number; sha256: string;
  created_at: string; content_url: string; data: DataInfo | null;
};
export type BoxApp = { app_id: string; work_id: string; title: string; kind: 'derived' | 'data'; file_ids: string[]; created_at: string };
export type PreviewColumn = { id: string; header: string; type: ColumnType; nullable: boolean; scale: number | null };
export type Preview = { format: 'csv' | 'xlsx'; sheet: string | null; rows: number; columns: PreviewColumn[]; sample: string[][] };
export type Card = { app_id: string; work_id: string; title: string };
export type Lineage = { box_id: string; box_name: string; files: { file_id: string; name: string; kind: FileKind; source_id: string; revision_id: string; content_url: string }[] };

/** A refusal: the server's code (or a local one) and the status. */
export class BoxError extends Error {
  constructor(readonly code: string, readonly status: number, readonly column = '') {
    super(code);
  }
}

const REFUSALS: Record<string, string> = {
  not_found: `That ${noun} or file is not available to you.`,
  file_too_large: 'That file is over 20 MB. Nothing was stored.',
  unsupported_file: 'That file type is not supported here, or its contents do not match its name. Nothing was stored.',
  box_full: `This ${noun} already holds 20 files. Nothing was stored.`,
  too_many_requests: 'Too many uploads in a short time. Try again in a moment.',
  data_refused: 'Those column types do not fit the file’s values. Nothing was registered.',
  not_a_folder_request: `That did not read as a request about this ${noun}.`,
  could_not_compose: `An app could not be built from this ${noun} right now. Nothing was placed.`,
  composer_unavailable: 'Building apps is not available on this server right now.',
  cross_site: 'That request was refused.',
  unavailable: 'The service is unavailable right now. Try again in a moment.',
  network: 'The request could not be sent. Check your connection.',
  malformed: 'That request was not understood.',
};

/** The neutral sentence for a refusal. */
export function refusalText(error: unknown): string {
  if (error instanceof BoxError) {
    if (error.code === 'data_refused' && error.column) return `Column “${error.column}” does not fit its type. Nothing was registered.`;
    return REFUSALS[error.code] ?? 'That could not be done right now.';
  }
  return REFUSALS.unavailable ?? '';
}

const str = (value: unknown): value is string => typeof value === 'string';
const num = (value: unknown): value is number => typeof value === 'number' && Number.isFinite(value);

export function box(value: unknown): Box | null {
  if (typeof value !== 'object' || value === null) return null;
  const v = value as Record<string, unknown>;
  if (!isUuid(v.box_id) || !num(v.number) || !str(v.name) || !['box', 'uploads', 'app'].includes(v.kind as string)) return null;
  return {
    box_id: v.box_id, number: v.number, name: v.name.slice(0, 120), kind: v.kind as Box['kind'],
    app_id: isUuid(v.app_id) ? v.app_id : null, owner: v.owner === true,
    file_count: num(v.file_count) ? v.file_count : 0, created_at: str(v.created_at) ? v.created_at : '',
  };
}

function dataInfo(value: unknown): DataInfo | null {
  if (typeof value !== 'object' || value === null) return null;
  const v = value as Record<string, unknown>;
  if (!isUuid(v.source_id) || !isUuid(v.revision_id) || !num(v.row_count)) return null;
  const fields = Array.isArray(v.fields)
    ? v.fields.flatMap((f) => (typeof f === 'object' && f !== null && str((f as Record<string, unknown>).id) ? [{ id: (f as Record<string, string>).id }] : []))
    : [];
  return { source_id: v.source_id, revision_id: v.revision_id, normalized_digest: str(v.normalized_digest) ? v.normalized_digest : '', row_count: v.row_count, fields };
}

/** The content route of a file, rebuilt from checked ids (never a server string). */
export function contentUrl(boxId: string, fileId: string): string {
  return `/v1/boxes/${boxId}/files/${fileId}/content`;
}

export function boxFile(value: unknown, boxId: string): BoxFile | null {
  if (typeof value !== 'object' || value === null) return null;
  const v = value as Record<string, unknown>;
  if (!isUuid(v.file_id) || !str(v.name) || !FILE_KINDS.includes(v.kind as FileKind)) return null;
  return {
    file_id: v.file_id, name: v.name.slice(0, 200), kind: v.kind as FileKind,
    media_type: str(v.media_type) ? v.media_type : '', bytes: num(v.bytes) ? v.bytes : 0,
    sha256: str(v.sha256) ? v.sha256 : '', created_at: str(v.created_at) ? v.created_at : '',
    content_url: contentUrl(boxId, v.file_id), data: dataInfo(v.data),
  };
}

function boxApp(value: unknown): BoxApp | null {
  if (typeof value !== 'object' || value === null) return null;
  const v = value as Record<string, unknown>;
  if (!isUuid(v.app_id) || !isUuid(v.work_id) || !str(v.title)) return null;
  return {
    app_id: v.app_id, work_id: v.work_id, title: v.title.slice(0, 200), kind: v.kind === 'data' ? 'data' : 'derived',
    file_ids: Array.isArray(v.file_ids) ? v.file_ids.filter(isUuid) : [], created_at: str(v.created_at) ? v.created_at : '',
  };
}

export function preview(value: unknown): Preview | { refused: string } | null {
  if (typeof value !== 'object' || value === null) return null;
  const v = value as Record<string, unknown>;
  if (str(v.refused)) return { refused: v.refused };
  if ((v.format !== 'csv' && v.format !== 'xlsx') || !Array.isArray(v.columns)) return null;
  const columns: PreviewColumn[] = [];
  for (const c of v.columns.slice(0, 32)) {
    if (typeof c !== 'object' || c === null) return null;
    const col = c as Record<string, unknown>;
    if (!str(col.id) || !/^[a-z][a-z0-9_]{0,63}$/.test(col.id) || !COLUMN_TYPES.includes(col.type as ColumnType)) return null;
    columns.push({
      id: col.id, header: str(col.header) ? col.header.slice(0, 120) : col.id, type: col.type as ColumnType,
      nullable: col.nullable === true, scale: num(col.scale) ? col.scale : null,
    });
  }
  const sample = Array.isArray(v.sample)
    ? v.sample.slice(0, 20).map((row) => (Array.isArray(row) ? row.slice(0, 32).map((cell) => (str(cell) ? cell.slice(0, 200) : '')) : []))
    : [];
  return { format: v.format, sheet: str(v.sheet) ? v.sheet : null, rows: num(v.rows) ? v.rows : 0, columns, sample };
}

function card(value: unknown): Card | null {
  if (typeof value !== 'object' || value === null) return null;
  const v = value as Record<string, unknown>;
  return isUuid(v.app_id) && isUuid(v.work_id) ? { app_id: v.app_id, work_id: v.work_id, title: str(v.title) ? v.title : 'Data' } : null;
}

export async function call(method: string, url: string, body?: unknown): Promise<Record<string, unknown>> {
  let response: Response;
  try {
    response = await fetch(url, {
      method,
      credentials: 'same-origin',
      cache: 'no-store',
      ...(body === undefined ? {} : { headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) }),
    });
  } catch {
    throw new BoxError('network', 0);
  }
  const json = (await response.json().catch(() => null)) as Record<string, unknown> | null;
  if (!response.ok) {
    throw new BoxError(str(json?.code) ? json.code : 'unavailable', response.status, str(json?.column) ? json.column : '');
  }
  if (!json) throw new BoxError('unavailable', response.status);
  return json;
}

export async function listBoxes(): Promise<Box[]> {
  const json = await call('GET', '/v1/boxes');
  return Array.isArray(json.boxes) ? json.boxes.map(box).filter((b): b is Box => b !== null) : [];
}

export async function createBox(name: string, appId?: string): Promise<Box> {
  const json = await call('POST', '/v1/boxes', appId ? { name, app_id: appId } : { name });
  const made = box(json.box);
  if (!made) throw new BoxError('unavailable', 500);
  return made;
}

/** Renames a folder the member owns. */
export async function renameBox(boxId: string, name: string): Promise<Box> {
  if (!isUuid(boxId)) throw new BoxError('not_found', 404);
  const json = await call('POST', `/v1/boxes/${boxId}/name`, { name });
  const made = box(json.box);
  if (!made) throw new BoxError('unavailable', 500);
  return made;
}

export async function uploadsBox(): Promise<Box> {
  const json = await call('POST', '/v1/boxes/uploads', {});
  const made = box(json.box);
  if (!made) throw new BoxError('unavailable', 500);
  return made;
}

export type BoxContents = { box: Box; files: BoxFile[]; apps: BoxApp[] };
export async function getBox(boxId: string): Promise<BoxContents> {
  if (!isUuid(boxId)) throw new BoxError('not_found', 404);
  const json = await call('GET', `/v1/boxes/${boxId}`);
  const found = box(json.box);
  if (!found) throw new BoxError('unavailable', 500);
  return {
    box: found,
    files: Array.isArray(json.files) ? json.files.map((f) => boxFile(f, boxId)).filter((f): f is BoxFile => f !== null) : [],
    apps: Array.isArray(json.apps) ? json.apps.map(boxApp).filter((a): a is BoxApp => a !== null) : [],
  };
}

export type Uploaded = { file: BoxFile; preview: Preview | { refused: string } | null };
/** One file, one multipart part named `file`, with progress (0..1). */
export function uploadFile(boxId: string, file: File, progress: (fraction: number) => void): Promise<Uploaded> {
  return new Promise((resolve, reject) => {
    if (!isUuid(boxId)) {
      reject(new BoxError('not_found', 404));
      return;
    }
    const form = new FormData();
    form.append('file', file, file.name);
    const xhr = new XMLHttpRequest();
    xhr.open('POST', `/v1/boxes/${boxId}/files`);
    xhr.responseType = 'text';
    xhr.upload.addEventListener('progress', (event) => {
      if (event.lengthComputable) progress(event.loaded / event.total);
    });
    xhr.addEventListener('error', () => reject(new BoxError('network', 0)));
    xhr.addEventListener('load', () => {
      let json: Record<string, unknown> | null = null;
      try {
        json = JSON.parse(xhr.responseText) as Record<string, unknown>;
      } catch {
        json = null;
      }
      if (xhr.status < 200 || xhr.status >= 300) {
        reject(new BoxError(str(json?.code) ? json.code : 'unavailable', xhr.status));
        return;
      }
      const made = boxFile(json?.file, boxId);
      if (!made) {
        reject(new BoxError('unavailable', xhr.status));
        return;
      }
      progress(1);
      resolve({ file: made, preview: preview(json?.preview) });
    });
    xhr.send(form);
  });
}

export async function fetchPreview(boxId: string, fileId: string): Promise<Preview | { refused: string } | null> {
  if (!isUuid(boxId) || !isUuid(fileId)) throw new BoxError('not_found', 404);
  return preview(await call('GET', `/v1/boxes/${boxId}/files/${fileId}/preview`));
}

export type ConfirmedColumn = { id: string; type: ColumnType; nullable: boolean; scale?: number };
export async function makeData(boxId: string, fileId: string, columns: ConfirmedColumn[], asCard: boolean): Promise<{ data: DataInfo | null; card: Card | null }> {
  if (!isUuid(boxId) || !isUuid(fileId)) throw new BoxError('not_found', 404);
  const json = await call('POST', `/v1/boxes/${boxId}/files/${fileId}/data`, { columns, card: asCard });
  return { data: dataInfo(json.data), card: card(json.card) };
}

export async function makeApp(boxId: string, purpose: string, fileIds?: string[]): Promise<{ app_id: string; work_id: string; title: string; files: number; timings: unknown }> {
  if (!isUuid(boxId)) throw new BoxError('not_found', 404);
  const json = await call('POST', `/v1/boxes/${boxId}/apps`, fileIds ? { purpose, file_ids: fileIds.filter(isUuid) } : { purpose });
  if (!isUuid(json.app_id) || !isUuid(json.work_id)) throw new BoxError('unavailable', 500);
  return { app_id: json.app_id, work_id: json.work_id, title: str(json.title) ? json.title : 'Generated app', files: num(json.files) ? json.files : 0, timings: json.timings };
}

export async function attachFile(boxId: string, fileId: string, appId: string, workId: string): Promise<{ box: Box | null; card: Card | null }> {
  if (![boxId, fileId, appId, workId].every(isUuid)) throw new BoxError('not_found', 404);
  const json = await call('POST', `/v1/boxes/${boxId}/files/${fileId}/attach`, { app_id: appId, work_id: workId });
  return { box: box(json.box), card: card(json.card) };
}

const lineageCache = new Map<string, Promise<Lineage | null>>();
/** The box a work was built from (cached per page; null: none or not readable). */
export function lineage(appId: string, workId: string): Promise<Lineage | null> {
  if (!isUuid(appId) || !isUuid(workId)) return Promise.resolve(null);
  const key = `${appId}/${workId}`;
  let found = lineageCache.get(key);
  if (!found) {
    found = call('GET', `/v1/boxes/lineage/${appId}/${workId}`).then((json) => {
      if (!isUuid(json.box_id)) return null;
      const boxId = json.box_id;
      const files = Array.isArray(json.files) ? json.files.flatMap((value) => {
        if (typeof value !== 'object' || value === null) return [];
        const f = value as Record<string, unknown>;
        if (!isUuid(f.file_id) || !str(f.name)) return [];
        return [{
          file_id: f.file_id, name: f.name.slice(0, 200), kind: (FILE_KINDS.includes(f.kind as FileKind) ? f.kind : 'csv') as FileKind,
          source_id: isUuid(f.source_id) ? f.source_id : '', revision_id: isUuid(f.revision_id) ? f.revision_id : '',
          content_url: contentUrl(boxId, f.file_id),
        }];
      }) : [];
      return { box_id: boxId, box_name: str(json.box_name) ? json.box_name.slice(0, 120) : 'Box', files };
    }).catch(() => null).then((result) => {
      // APP-113: only a found lineage is kept; a miss or a failure asks again next time.
      if (!result) lineageCache.delete(key);
      return result;
    });
    lineageCache.set(key, found);
  }
  return found;
}

/** "12.4 KB": the canvas's one byte format, steps of 1000 (`display.ts` `amount`, as the server's `canvas_display`). */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '';
  return amount(true, false, BigInt(Math.floor(bytes)))[0];
}

/** Whether a local file is tabular data (by name; the server judges the bytes). */
export function isDataFile(file: File): boolean {
  return /\.(csv|xlsx)$/i.test(file.name);
}

/** The preview's columns as the confirmation the server expects (the defaults). */
export function defaultColumns(p: Preview): ConfirmedColumn[] {
  return p.columns.map((c) => ({ id: c.id, type: c.type, nullable: c.nullable, ...(c.scale !== null ? { scale: c.scale } : {}) }));
}

// --- The Brief (AI brief, cited from the folder's files) ---

export type CitePart = { text: string; cite: { file_id: string; name: string; page: number | null } };
export type ValuePart = { value: string; exact: string; view_id: string; view_title: string; receipt_digest: string; app_id: string; work_id: string };
export type BriefPart = { text: string } | CitePart | ValuePart;
export type BriefFile = { file_id: string; name: string; kind: FileKind; status: 'included' | 'too_large' | 'names_only'; paragraph: BriefPart[] };
export type Brief = { state: 'ready' | 'none'; label: string; generated_at: string | null; overview: BriefPart[]; files: BriefFile[]; key_points: BriefPart[][] };

function briefPart(value: unknown): BriefPart | null {
  if (typeof value !== 'object' || value === null) return null;
  const p = value as Record<string, unknown>;
  if (str(p.value) && str(p.view_id) && /^[a-z][a-z0-9_]{0,63}$/.test(p.view_id)) {
    return {
      value: p.value.slice(0, 80), exact: str(p.exact) ? p.exact.slice(0, 80) : '', view_id: p.view_id,
      view_title: str(p.view_title) ? p.view_title.slice(0, 120) : p.view_id, receipt_digest: str(p.receipt_digest) ? p.receipt_digest.slice(0, 80) : '',
      app_id: isUuid(p.app_id) ? p.app_id : '', work_id: isUuid(p.work_id) ? p.work_id : '',
    };
  }
  if (!str(p.text)) return null;
  const cite = p.cite as Record<string, unknown> | undefined;
  if (cite && typeof cite === 'object' && isUuid(cite.file_id)) {
    return {
      text: p.text.slice(0, 800),
      cite: { file_id: cite.file_id, name: str(cite.name) ? cite.name.slice(0, 200) : 'file', page: num(cite.page) && cite.page >= 1 ? Math.floor(cite.page) : null },
    };
  }
  return { text: p.text.slice(0, 800) };
}
const parts = (value: unknown, max = 40): BriefPart[] =>
  Array.isArray(value) ? value.slice(0, max).map(briefPart).filter((p): p is BriefPart => p !== null) : [];

function briefOf(value: unknown): Brief | null {
  if (typeof value !== 'object' || value === null) return null;
  const v = value as Record<string, unknown>;
  if (v.state !== 'ready' && v.state !== 'none') return null;
  const files = Array.isArray(v.files) ? v.files.slice(0, 20).flatMap((value_): BriefFile[] => {
    if (typeof value_ !== 'object' || value_ === null) return [];
    const f = value_ as Record<string, unknown>;
    if (!isUuid(f.file_id) || !str(f.name)) return [];
    const status = f.status === 'too_large' || f.status === 'names_only' ? f.status : 'included';
    return [{ file_id: f.file_id, name: f.name.slice(0, 200), kind: FILE_KINDS.includes(f.kind as FileKind) ? f.kind as FileKind : 'text', status, paragraph: parts(f.paragraph) }];
  }) : [];
  return {
    state: v.state, label: str(v.label) ? v.label.slice(0, 80) : 'AI brief · cited from your files',
    generated_at: str(v.generated_at) ? v.generated_at : null, overview: parts(v.overview), files,
    key_points: Array.isArray(v.key_points) ? v.key_points.slice(0, 12).map((point) => parts(point)).filter((point) => point.length > 0) : [],
  };
}

/** `GET` the folder's brief, or `POST` to (re)generate it. */
export async function brief(boxId: string, generate: boolean): Promise<Brief> {
  if (!isUuid(boxId)) throw new BoxError('not_found', 404);
  const json = await call(generate ? 'POST' : 'GET', `/v1/boxes/${boxId}/brief`, generate ? {} : undefined);
  const found = briefOf(json);
  if (!found) throw new BoxError('unavailable', 500);
  return found;
}
