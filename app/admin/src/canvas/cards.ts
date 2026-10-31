// APP-123: cards made by you and pinned answers, the server side of them
// (`composer::cards`, migration 0015). No DOM here (the tests run it as is):
// the canvas places them (`shape.ts` a made card, `ask.ts` a pinned answer).
//
// A made card is a view a member made with Shape (**Add as new card**, or
// **Make it a card** on an Ask answer): its app and work, the card it came
// from, and its closed view spec, which the server judged against the
// work's published schema (never the client's field list) before storing
// it. It stores no record value: the canvas reads the records live, with
// their receipts. A pinned answer is what Ask answered, with its storage
// receipts (checked by the server against the record revisions); it never
// runs again. Both are the member's own ("made by you") and persist across
// sessions; the app's owner or an admin can add a made card for everyone
// who can read the app's work, and take another member's card added for
// everyone back to its maker or remove it (`can_manage`). Removing is a soft
// delete on the server. A pinned answer's text is not derived again: it is
// labeled "As answered on <date>" (`answeredLine`) so it never passes as a
// live answer.
//
// Every stored card the server sends is judged again here (`storedCard`,
// `madeView`, `pinnedAnswer`) before any use, as Shape's specs are.
import { cardView, type CardView } from './talk-view';

export const CARDS_URL = '/v1/cards';
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);

export type StoredCard = {
  card_id: string; kind: 'card' | 'answer'; app_id: string | null; work_id: string | null; source: string; address: string; title: string;
  spec: unknown; everyone: boolean; mine: boolean; can_share: boolean; can_manage: boolean; created_at: string;
};
/** Where a made card reads its data: its records card's collection, or its view card's view. */
export type From = { collection: string } | { view: string };
export type Reference = { ref: string; app_id: string; work_id: string; record_id: string; address: string; label: string };
export type Receipt = Record<string, unknown>;
export type Answer = { question: string; text: string; scope: { kind: 'card' | 'app' | 'canvas'; address: string }; references: Reference[]; receipts: Receipt[] };

/** A card from `GET /v1/cards`, judged again; null drops it. */
export function storedCard(value: unknown): StoredCard | null {
  if (!isObject(value)) return null;
  const { card_id: id, kind, app_id: app, work_id: work, source, address, title, everyone, mine, can_share: share, can_manage: manage = false, created_at: created } = value;
  const optionalId = (v: unknown) => v === null || (typeof v === 'string' && UUID.test(v));
  if (typeof id !== 'string' || !UUID.test(id) || (kind !== 'card' && kind !== 'answer') || !optionalId(app) || !optionalId(work)
    || typeof source !== 'string' || typeof address !== 'string' || !/^[a-z0-9:.-]{0,88}$/.test(address)
    || typeof title !== 'string' || !title || title.length > 120 || typeof everyone !== 'boolean' || typeof mine !== 'boolean'
    || typeof share !== 'boolean' || typeof manage !== 'boolean' || typeof created !== 'string') return null;
  return {
    card_id: id, kind, app_id: app as string | null, work_id: work as string | null, source, address, title, spec: value.spec,
    everyone, mine, can_share: share, can_manage: manage && !mine && everyone, created_at: created,
  };
}

/** A made card's data source and view (its spec judged again, the answer's record set allowed). */
export function madeView(card: StoredCard): { from: From; view: CardView } | null {
  if (card.kind !== 'card' || !card.app_id || !card.work_id || !isObject(card.spec)) return null;
  const { from, view } = card.spec;
  if (!isObject(from) || Object.keys(card.spec).length !== 2) return null;
  const judged = cardView(view, { stored: true });
  if (!judged) return null;
  if (typeof from.collection === 'string' && UUID.test(from.collection) && Object.keys(from).length === 1) return { from: { collection: from.collection }, view: judged };
  if (typeof from.view === 'string' && /^[a-z][a-z0-9_-]{0,63}$/.test(from.view) && Object.keys(from).length === 1 && !judged.records) return { from: { view: from.view }, view: judged };
  return null;
}

/** A pinned answer's question, text, scope, references and receipts, judged again. */
export function pinnedAnswer(card: StoredCard): Answer | null {
  if (card.kind !== 'answer' || !isObject(card.spec)) return null;
  const { question, text, scope, references, receipts } = card.spec;
  if (typeof question !== 'string' || typeof text !== 'string' || !isObject(scope) || !Array.isArray(references) || !Array.isArray(receipts)) return null;
  const kind = scope.kind;
  if ((kind !== 'card' && kind !== 'app' && kind !== 'canvas') || typeof scope.address !== 'string') return null;
  const refs = references.filter((r): r is Reference => isObject(r) && typeof r.record_id === 'string' && typeof r.work_id === 'string');
  return { question, text, scope: { kind, address: scope.address }, references: refs, receipts: receipts.filter(isObject) };
}

/** A fresh id for a card the member is about to store (a retry sends the same one). */
export function newCardId(): string {
  return crypto.randomUUID();
}

/** `POST /v1/cards` for a made card. */
export function madeCardRequest(card: {
  id: string; app: string; work: string; source: string; address: string; title: string; from: From; view: CardView;
}): Record<string, unknown> {
  return {
    card_id: card.id, kind: 'card', app_id: card.app, work_id: card.work, card: card.source, address: card.address,
    title: card.title.trim().slice(0, 120), ...card.from, spec: card.view,
  };
}

/** `POST /v1/cards` for a pinned answer: exactly what Ask answered. */
export function pinRequest(id: string, answer: Answer): Record<string, unknown> {
  return { card_id: id, kind: 'answer', question: answer.question, text: answer.text, scope: answer.scope, references: answer.references, receipts: answer.receipts };
}

const REFUSALS: Record<string, string> = {
  too_many_cards: 'You already keep 100 cards and answers. Remove one first.',
  card_exists: 'That card was already saved with different contents.',
  unknown_receipt: 'One of the answer’s receipts no longer matches the records, so it was not pinned. Ask again, then pin.',
  not_shareable: 'A pinned answer is yours alone; only a card can be added for everyone.',
  drops_receipts: 'That view would hide its receipts, so it was not saved.',
  unknown_field: 'That view names a field the card does not have, so it was not saved.',
  invalid_request: 'That card could not be saved. Nothing was added.',
  too_many_requests: 'Too many changes in a short time. Try again in a moment.',
  payload_too_large: 'That answer is too large to pin.',
};

async function send(method: 'GET' | 'POST', url: string, body?: Record<string, unknown>): Promise<Record<string, unknown> | string> {
  let response: Response;
  try {
    response = await fetch(url, {
      method, credentials: 'same-origin', cache: 'no-store',
      ...(body ? { headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) } : {}),
    });
  } catch {
    return 'It could not be sent. Check your connection; nothing changed.';
  }
  const answer = (await response.json().catch(() => null)) as Record<string, unknown> | null;
  if (response.status === 404) return 'You can’t open that app or card.';
  if (response.status === 413) return REFUSALS.payload_too_large ?? '';
  if (!response.ok || !answer) {
    const code = typeof answer?.code === 'string' ? answer.code : '';
    return REFUSALS[code] ?? (code ? `Not saved (${code.replace(/_/g, ' ')}). Nothing changed.` : 'It could not be saved right now. Nothing changed.');
  }
  return answer;
}

/** The member's cards and pinned answers (their own and those added for everyone). */
export async function listCards(): Promise<StoredCard[]> {
  const answer = await send('GET', CARDS_URL);
  if (typeof answer === 'string' || !Array.isArray(answer.cards)) return [];
  return answer.cards.map(storedCard).filter((card): card is StoredCard => card !== null);
}

/** Stores a made card or a pinned answer: the stored card, or why not. */
export async function storeCard(body: Record<string, unknown>): Promise<StoredCard | string> {
  const answer = await send('POST', CARDS_URL, body);
  if (typeof answer === 'string') return answer;
  return storedCard(answer.card) ?? 'The saved card came back in the wrong shape.';
}

/** Adds the member's card for everyone who can open its app (the owner or an admin), or keeps it for them. */
export async function shareCard(id: string, everyone: boolean): Promise<StoredCard | string> {
  if (!UUID.test(id)) return 'That card cannot be shared.';
  const answer = await send('POST', `${CARDS_URL}/${id}/everyone`, { everyone });
  if (typeof answer === 'string') return answer;
  return storedCard(answer.card) ?? 'The card came back in the wrong shape.';
}

/** Removes the member's card or pinned answer (for everyone, when it was added for everyone). */
export async function removeCard(id: string): Promise<true | string> {
  if (!UUID.test(id)) return 'That card cannot be removed.';
  const answer = await send('POST', `${CARDS_URL}/${id}/remove`, {});
  return typeof answer === 'string' ? answer : true;
}

/**
 * The line a pinned answer shows: when it was answered, about what, and that
 * it does not run again. Its text is what Ask said then, never derived
 * again, so it is never presented as a live answer.
 */
export function answeredLine(card: Pick<StoredCard, 'created_at'>, origin: string): string {
  const when = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(card.created_at)
    ? `${card.created_at.slice(0, 10)} ${card.created_at.slice(11, 16)} UTC` : 'an earlier date';
  return `As answered on ${when} · asked about ${origin} · it does not run again`;
}

/** The line a made card shows: who made it and whether everyone sees it. */
export function madeLine(card: Pick<StoredCard, 'mine' | 'everyone' | 'address'>): string {
  if (card.mine && card.everyone) return `Made by you from /${card.address} · added for everyone`;
  if (card.mine) return `Made by you from /${card.address} · only you see it`;
  return `From /${card.address} · added for everyone by the app’s owner`;
}
