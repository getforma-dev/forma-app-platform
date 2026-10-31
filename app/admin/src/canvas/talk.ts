// APP-122: the two AI jobs wired into the canvas. Where the member types
// decides the job, never the AI: `?` first in the bar (or the Ask tab) is
// Ask (`ask.ts`); `/shape <address> <words>` and `/<app>.<card> <words>` are
// Shape (`shape.ts`). Everything else in the bar is unchanged: Do
// (`/<app> <words>`), Forma (a plain sentence), Place (`/<name>`).
import { setTalkRouter } from './actions';
import { askAboutItem, openAsk, startAsk } from './ask';
import { fillBar } from './address-chip';
import { registerCommands, type CanvasCommand } from './command-registry';
import { setTalkActions } from './controls';
import { addressOfItem, runDot, SHAPE_COMMAND, startShape } from './shape';
import { barIntent } from './talk-view';

/** `/ask <question>`: the same as `? <question>` (listed as `? <question>`). */
export const ASK_COMMAND: CanvasCommand = {
  slug: 'ask',
  title: 'Ask',
  purpose: 'A read-only question about your data, answered with receipts; changes nothing',
  run: (text: string, report?: (message: string) => void) => {
    const input = document.getElementById('intent-input') as HTMLInputElement | null;
    if (input) input.value = '';
    openAsk(text);
    report?.(text.trim() ? 'Asked. The answer is in the Ask tab.' : 'Type your question in the Ask tab.');
  },
};

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
  registerCommands([SHAPE_COMMAND, ASK_COMMAND]);
  setTalkRouter(routeTalk);
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
