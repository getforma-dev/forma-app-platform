// APP-122: the two AI jobs wired into the canvas. Where the member types
// decides the job, never the AI: `?` first in the bar (or the Ask tab) is
// Ask (`ask.ts`), as is `/ask [address] <question>`; `/shape <address>
// <words>`, `/<app>.<card> <words>` and its full form `/card:<app>.<card>
// <words>` are Shape (`shape.ts`). Everything else in the bar is unchanged: Do
// (`/<app> <words>`), Forma (a plain sentence), Place (`/<name>`).
//
// APP-123: `/forma <words>` is a plain sentence typed as a verb (Forma picks
// or builds an app), and the member's cards made by Shape and pinned
// answers are placed again on every load (`cards.ts`).
//
// APP-124: a folder's **Ask** button opens the Ask tab scoped to the folder.
import { setTalkRouter, submitSentence } from './actions';
import { askAboutItem, askCompletions, askLine, folderScope, openAsk, placePinnedAnswer, startAsk } from './ask';
import { setFolderAsk } from './boxes';
import { cardInventory, onStoredCardChange, type StoredCard, type StoredCardChange } from './cards';
import { setSavedCardInventory } from './library';
import { canvasApi, removeUtilityItem } from './board';
import { fillBar } from './address-chip';
import { registerCommands, type CanvasCommand } from './command-registry';
import { setTalkActions } from './controls';
import { addressOfItem, placeMadeCard, runDot, SHAPE_COMMAND, startShape } from './shape';
import { registerCompleter } from './slash';
import { barIntent } from './talk-view';

/**
 * `/ask [address] <question>` (`askLine`): scoped to the address when the
 * first word is one of the member's app or card names, else to the
 * selection or the canvas, exactly as `? <question>` is.
 */
export const ASK_COMMAND: CanvasCommand = {
  slug: 'ask',
  title: 'Ask',
  purpose: 'A read-only question about your data, answered with receipts; scoped to an app or card when you name one first; changes nothing',
  run: (text: string, report?: (message: string) => void) => {
    const line = askLine(text);
    if (line.message) {
      report?.(line.message);
      return;
    }
    const input = document.getElementById('intent-input') as HTMLInputElement | null;
    if (input) input.value = '';
    openAsk(line.question, line.scope);
    const where = line.scope ? ` about /${line.scope.address}` : '';
    report?.(line.question.trim() ? `Asked${where}. The answer is in the Ask tab.` : `Type your question${where} in the Ask tab.`);
  },
};

/**
 * APP-123: `/forma <words>` is exactly a plain sentence: Forma picks or
 * builds an app from the words, behind its confirm card (`submitSentence`).
 */
export const FORMA_COMMAND: CanvasCommand = {
  slug: 'forma',
  title: 'Forma',
  purpose: 'Make something new: picks or builds an app from your words, exactly as a plain sentence does; confirms first',
  run: async (text: string, report: (message: string) => void = () => undefined) => {
    const words = text.trim();
    if (!words || words.startsWith('/') || words.startsWith('?')) {
      report('Say what you need after /forma, e.g. /forma track client dinners over $200.');
      return;
    }
    const input = document.getElementById('intent-input') as HTMLInputElement | null;
    if (input) input.value = '';
    await submitSentence(words, report);
  },
};

const savedCards = new Map<string, StoredCard>();
let inventoryRequest: Promise<boolean> | null = null;
let initialCards: Promise<number> | null = null;
let observeCards = false;
const pendingChanges = new Map<string, StoredCard | null>();
const showingCards = new Map<string, Promise<string>>();
type SavedPlacement = {shown: boolean; message: string};
const savedPlacements = new Map<string, Promise<SavedPlacement>>();
function savedUtility(card: StoredCard): HTMLElement | null {
  return document.querySelector<HTMLElement>(`[data-utility="utility-${card.kind === 'card' ? 'made' : 'pin'}-${card.card_id}"]`);
}
function hideSavedUtility(card: StoredCard): void {
  const item = savedUtility(card);
  if (item) removeUtilityItem(item);
}
function publishSavedCards(message = ''): void {
  setSavedCardInventory([...savedCards.values()].map(card => ({id: card.card_id, title: card.title, kind: card.kind, createdAt: card.created_at})), showSavedCard, message, refreshSavedCardList);
}
async function refreshSavedCardList(): Promise<string> {
  return await refreshSavedCards() ? 'Saved cards refreshed.' : 'Saved cards could not be refreshed. Use Refresh saved cards to retry; listed metadata may be out of date.';
}
function acceptSavedChange(change: StoredCardChange): void {
  const id = 'removed' in change ? change.removed : change.card.card_id;
  const card = 'card' in change && (change.card.mine || change.card.everyone) ? change.card : null;
  if (inventoryRequest) pendingChanges.set(id, card);
  const previous = savedCards.get(id);
  if (card) savedCards.set(id, card); else savedCards.delete(id);
  if (!card && previous) hideSavedUtility(previous);
  publishSavedCards();
}
/** Initial load and explicit Show only; canvas movement never polls inventory. */
async function refreshSavedCards(): Promise<boolean> {
  if (inventoryRequest) return inventoryRequest;
  inventoryRequest = (async () => {
    const result = await cardInventory();
    if (!result.ok) {publishSavedCards(result.message); return false;}
    const previous = [...savedCards.values()];
    savedCards.clear();
    for (const card of result.cards) savedCards.set(card.card_id, card);
    // A confirmed mutation during this read wins over its older response.
    for (const [id, card] of pendingChanges) {if (card) savedCards.set(id, card); else savedCards.delete(id);}
    // Only a successful authorized inventory can withdraw existing displays.
    for (const card of previous) if (!savedCards.has(card.card_id)) hideSavedUtility(card);
    publishSavedCards();
    return true;
  })().finally(() => {inventoryRequest = null; pendingChanges.clear();});
  return inventoryRequest;
}
function showSavedCard(id: string): Promise<string> {
  const pending = showingCards.get(id);
  if (pending) return pending;
  const request = showSavedCardNow(id).finally(() => {showingCards.delete(id);});
  showingCards.set(id, request);
  return request;
}
async function showSavedCardNow(id: string): Promise<string> {
  // Reauthorize a historical answer before displaying its stored text again.
  if (!await refreshSavedCards()) return 'Saved cards could not be refreshed. Nothing new was shown.';
  return (await placeSavedCard(id, true)).message;
}
/** Startup restoration and Show share one queue: an older load can never
 * clean up the newer placement of the same durable card ID. */
function placeSavedCard(id: string, focus: boolean): Promise<SavedPlacement> {
  const prior = savedPlacements.get(id) ?? Promise.resolve(null);
  const request = prior.catch(() => null).then(async (): Promise<SavedPlacement> => {
    const card = savedCards.get(id);
    if (!card) return {shown: false, message: 'That saved card is no longer available to you.'};
    let item = savedUtility(card);
    if (!item) {
      const shown = card.kind === 'card' ? await placeMadeCard(card) : placePinnedAnswer(card);
      if (!shown) return {shown: false, message: 'That saved card could not be shown.'};
      item = savedUtility(card);
    }
    if (savedCards.get(id) !== card) {
      if (item) removeUtilityItem(item);
      return {shown: false, message: 'That saved card changed while it was opening. Try Show again.'};
    }
    if (item && focus) canvasApi()?.centerItem(item);
    return {shown: Boolean(item), message: item ? `${card.title} is on your canvas.` : 'That saved card could not be shown.'};
  });
  savedPlacements.set(id, request);
  void request.finally(() => {if (savedPlacements.get(id) === request) savedPlacements.delete(id);}).catch(() => undefined);
  return request;
}
/** APP-123: retain existing one-time restoration and publish its same inventory. */
export function placeMemberCards(): Promise<number> {
  if (initialCards) return initialCards;
  if (!observeCards) {onStoredCardChange(acceptSavedChange); observeCards = true;}
  publishSavedCards('Loading saved cards and answers…');
  initialCards = (async () => {
    if (!await refreshSavedCards()) return 0;
    let placed = 0;
    for (const id of [...savedCards.keys()]) if ((await placeSavedCard(id, false)).shown) placed += 1;
    return placed;
  })();
  return initialCards;
}

/** The bar's Ask and dot lanes (exported for the tests). */
export async function routeTalk(text: string, report: (message: string) => void): Promise<boolean> {
  const intent = barIntent(text);
  if (!intent) return false;
  if (intent.kind === 'ask') {
    const input = document.getElementById('intent-input') as HTMLInputElement | null;
    if (input) input.value = '';
    openAsk(intent.question);
    report(intent.question ? 'Asked. The answer is in the Ask tab; nothing on the canvas changes.' : 'Type your question in the Ask tab.');
    return true;
  }
  await runDot(intent.target, intent.words, report);
  return true;
}

export function startTalk(document_: Document): void {
  startAsk(document_);
  startShape();
  registerCommands([SHAPE_COMMAND, ASK_COMMAND, FORMA_COMMAND]);
  registerCompleter('ask', askCompletions, { select: false });
  setTalkRouter(routeTalk);
  // APP-124: a folder's **Ask** opens the Ask tab scoped to the folder's tables.
  setFolderAsk((boxId, name) => {
    const scope = folderScope(boxId, name);
    if (scope) openAsk('', scope);
  });
  void placeMemberCards();
  setTalkActions({
    ask: (item) => askAboutItem(item),
    shape: (item) => {
      const address = addressOfItem(item);
      if (!address) return false;
      fillBar(`/shape ${address} `, document_);
      return true;
    },
  });
}
