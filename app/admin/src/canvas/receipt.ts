// The intent bar's receipt attachment (APP-102): a separate control wired
// onto the server-rendered intent form (attach button, drag and drop onto
// the bar, paste into it). The file is inspected here before anything is
// sent (the upload pattern of Forma Generative UI's local-data client):
// not empty, at most 5 MB, and a JPEG, PNG or WebP by its magic bytes. HEIC
// (and an oversized photo) is converted in the browser to JPEG through a
// canvas when the browser can decode it, refused otherwise. The bytes stay
// in memory, and the action path (`actions.ts`) sends the image once, multipart, to
// `POST /v1/act`. The thumbnail is a small JPEG `data:` URL drawn here (the
// page's policy allows `data:` images, not `blob:`); a file this browser
// cannot decode is refused before upload. Nothing is stored (DS-56).
import { el } from './render';

export const MAX_RECEIPT_BYTES = 5 * 1024 * 1024;
/** The words the bar shows for a receipt sent without any (sent as none). */
export const RECEIPT_PROMPT = 'Here is my receipt';
export const RECEIPT_REFUSED = 'That file can’t be read as a receipt. Attach a JPEG, PNG or WebP photo under 5 MB.';

export type PendingReceipt = { blob: Blob; name: string; thumb: string };
let pending: PendingReceipt | null = null;

type Kind = 'jpeg' | 'png' | 'webp' | 'heic' | null;
/** The image kind from its first bytes (the declared type is not trusted). */
export function magic(bytes: Uint8Array): Kind {
  const at = (i: number, ...values: number[]) => values.every((v, j) => bytes[i + j] === v);
  const ascii = (i: number, text: string) => at(i, ...Array.from(text, (c) => c.charCodeAt(0)));
  if (at(0, 0xff, 0xd8, 0xff)) return 'jpeg';
  if (at(0, 0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a)) return 'png';
  if (ascii(0, 'RIFF') && ascii(8, 'WEBP')) return 'webp';
  if (ascii(4, 'ftyp') && ['heic', 'heix', 'heif', 'mif1', 'msf1', 'hevc'].some((brand) => ascii(8, brand))) return 'heic';
  return null;
}

/** HEIC or an oversized photo → JPEG ≤ 2400 px, when this browser decodes it. */
async function toJpeg(file: Blob): Promise<Blob | null> {
  try {
    const bitmap = await createImageBitmap(file);
    const scale = Math.min(1, 2400 / Math.max(bitmap.width, bitmap.height));
    const canvas = document.createElement('canvas');
    canvas.width = Math.round(bitmap.width * scale);
    canvas.height = Math.round(bitmap.height * scale);
    canvas.getContext('2d')?.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    bitmap.close();
    return await new Promise((resolve) => canvas.toBlob((blob) => resolve(blob), 'image/jpeg', 0.85));
  } catch {
    return null;
  }
}

/** A small JPEG `data:` thumbnail; null when this browser cannot decode the image. */
async function thumbnail(blob: Blob): Promise<string | null> {
  try {
    const bitmap = await createImageBitmap(blob);
    const scale = Math.min(1, 240 / Math.max(bitmap.width, bitmap.height));
    const canvas = document.createElement('canvas');
    canvas.width = Math.max(1, Math.round(bitmap.width * scale));
    canvas.height = Math.max(1, Math.round(bitmap.height * scale));
    canvas.getContext('2d')?.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    bitmap.close();
    return canvas.toDataURL('image/jpeg', 0.8);
  } catch {
    return null;
  }
}

/** Gates a file into a receipt, or answers why not. */
export async function inspect(file: File): Promise<PendingReceipt | string> {
  if (file.size === 0) return RECEIPT_REFUSED;
  const kind = magic(new Uint8Array(await file.slice(0, 16).arrayBuffer()));
  if (!kind) return RECEIPT_REFUSED;
  let blob: Blob = file;
  if (kind === 'heic' || file.size > MAX_RECEIPT_BYTES) {
    const converted = await toJpeg(file);
    if (!converted || converted.size > MAX_RECEIPT_BYTES) {
      return kind === 'heic' ? 'This browser can’t convert that HEIC photo. Export it as JPEG and attach it again.' : RECEIPT_REFUSED;
    }
    blob = converted;
  }
  const thumb = await thumbnail(blob);
  if (!thumb) return RECEIPT_REFUSED;
  return { blob, name: file.name || 'receipt', thumb };
}

// The attachment's own status line (the bar's line belongs to its island:
// writing into it would detach the island's reactive text).
let sayTimer = 0;
function say(document_: Document, message: string): void {
  const status = document_.querySelector<HTMLElement>('[data-receipt-status]');
  if (!status) return;
  status.textContent = message;
  status.classList.toggle('is-shown', Boolean(message));
  window.clearTimeout(sayTimer);
  if (message) sayTimer = window.setTimeout(() => say(document_, ''), 6000);
}

function chip(document_: Document): HTMLElement | null {
  return document_.querySelector<HTMLElement>('[data-receipt-chip]');
}

function show(document_: Document): void {
  const host = chip(document_);
  if (!host) return;
  if (!pending) {
    host.hidden = true;
    host.replaceChildren();
    document_.getElementById('intent-form')?.removeAttribute('data-receipt');
    return;
  }
  const remove = el(document_, 'button', { type: 'button', class: 'cv-attach-remove', 'aria-label': 'Remove the receipt' }, '×');
  remove.addEventListener('click', () => {
    clearReceipt(document_);
    document_.getElementById('intent-input')?.focus();
  });
  host.replaceChildren(el(document_, 'img', { src: pending.thumb, alt: 'Receipt to send', class: 'cv-attach-thumb' }), remove);
  host.hidden = false;
  document_.getElementById('intent-form')?.setAttribute('data-receipt', 'attached');
}

/** Attaches `file` after inspection; fills the bar when it is empty. */
export async function attach(document_: Document, file: File): Promise<void> {
  const result = await inspect(file);
  if (typeof result === 'string') {
    say(document_, result);
    return;
  }
  clearReceipt(document_);
  pending = result;
  show(document_);
  const input = document_.getElementById('intent-input') as HTMLInputElement | null;
  if (input && !input.value.trim()) {
    input.value = RECEIPT_PROMPT;
    input.select();
  }
  input?.focus();
  say(document_, 'Receipt attached. Add a note or who approves, then send. The photo is read once and not stored.');
}

/** The attached receipt, detached from the bar. */
export function takeReceipt(document_: Document): PendingReceipt | null {
  const taken = pending;
  pending = null;
  show(document_);
  return taken;
}
/** Puts a taken receipt back (Edit, or a refusal the member may retry). */
export function restoreReceipt(document_: Document, receipt: PendingReceipt): void {
  pending = receipt;
  show(document_);
}
export function clearReceipt(document_: Document): void {
  pending = null;
  show(document_);
}

/** Adds the attach control to the intent form once: button, drop, paste. */
export function wireReceiptAttach(document_: Document): void {
  const form = document_.getElementById('intent-form') as HTMLFormElement | null;
  const input = document_.getElementById('intent-input');
  if (!form || !input || form.dataset.attach === 'wired') return;
  form.dataset.attach = 'wired';
  const picker = el(document_, 'input', { type: 'file', accept: 'image/jpeg,image/png,image/webp,image/heic,image/heif,.heic,.heif', hidden: '', 'data-receipt-input': '' });
  const button = el(document_, 'button', { type: 'button', class: 'cv-attach', 'aria-label': 'Attach a receipt photo', title: 'Attach a receipt photo (or drop or paste one)' });
  // A paperclip, built as nodes (the page enforces Trusted Types).
  const svg = document_.createElementNS('http://www.w3.org/2000/svg', 'svg');
  for (const [name, value] of [['viewBox', '0 0 20 20'], ['width', '18'], ['height', '18'], ['aria-hidden', 'true']]) svg.setAttribute(name, value);
  const path = document_.createElementNS('http://www.w3.org/2000/svg', 'path');
  for (const [name, value] of [['d', 'M13.5 6.5l-5.8 5.8a1.6 1.6 0 002.3 2.3l6-6a3 3 0 00-4.3-4.3l-6.2 6.2a4.4 4.4 0 006.2 6.2l5.3-5.3'], ['fill', 'none'], ['stroke', 'currentColor'], ['stroke-width', '1.5'], ['stroke-linecap', 'round']]) path.setAttribute(name, value);
  svg.append(path);
  button.append(svg);
  const host = el(document_, 'span', { class: 'cv-attach-chip', hidden: '', 'data-receipt-chip': '' });
  const status = el(document_, 'p', { class: 'cv-intent-status', role: 'status', 'data-receipt-status': '' });
  input.before(button);
  input.after(host);
  form.append(picker, status);
  form.addEventListener('submit', () => say(document_, ''));
  button.addEventListener('click', () => picker.click());
  picker.addEventListener('change', () => {
    const file = picker.files?.[0];
    picker.value = '';
    if (file) void attach(document_, file);
  });
  for (const name of ['dragenter', 'dragover']) {
    form.addEventListener(name, (event) => {
      if (!(event as DragEvent).dataTransfer?.types.includes('Files')) return;
      event.preventDefault();
      form.dataset.dragging = 'true';
    });
  }
  for (const name of ['dragleave', 'drop']) {
    form.addEventListener(name, (event) => {
      if (!(event as DragEvent).dataTransfer?.types.includes('Files')) return;
      event.preventDefault();
      form.dataset.dragging = 'false';
    });
  }
  form.addEventListener('drop', (event) => {
    const file = (event as DragEvent).dataTransfer?.files[0];
    if (file) void attach(document_, file);
  });
  input.addEventListener('paste', (event) => {
    const file = Array.from((event as ClipboardEvent).clipboardData?.files ?? []).find((f) => f.type.startsWith('image/'));
    if (!file) return;
    event.preventDefault();
    void attach(document_, file);
  });
}
