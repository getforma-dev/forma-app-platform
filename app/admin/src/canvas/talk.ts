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
import { setTalkRouter, submitSentence } from './actions';
import { askAboutItem, askCompletions, askLine, openAsk, placePinnedAnswer, startAsk } from './ask';
import { listCards } from './cards';
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

/** APP-123: places the member's made cards and pinned answers (their own and those added for everyone). */
export async function placeMemberCards(): Promise<number> {
  let placed = 0;
  for (const card of await listCards()) {
    const shown = card.kind === 'card' ? await placeMadeCard(card) : placePinnedAnswer(card);
    if (shown) placed += 1;
  }
  return placed;
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
