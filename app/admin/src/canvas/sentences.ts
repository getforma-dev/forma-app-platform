// APP-112 plain words: every canvas command whose action can be said as a
// sentence also works as one in the intent bar ("unite the expense app",
// "take a note: call the auditor", "show the weather in Austin", "new folder
// Q3 pack"). These are canvas actions only (nothing here writes a record or
// reaches the model): a sentence that matches none of them goes on to the
// action path and the composer as before. The patterns are anchored and
// narrow, so an expense ("I spent $86 on a note pad") or an app request
// ("build a weather tracker") never lands here.
import { openLibrary } from './library';
import { runSlash } from './slash';
import { toggleTimings } from './timing';

type Report = (message: string) => void;
type Rule = { match: RegExp; run: (found: RegExpMatchArray, report: Report) => Promise<void> | void };

const slash = (command: string) => (found: RegExpMatchArray, report: Report) => runSlash(`/${command}${found[1] ? ` ${found[1].trim()}` : ''}`, report);
const APP = '(?:the\\s+|my\\s+)?(.+?)(?:\\s+app)?(?:\\s+cards)?';

/** Sentence → canvas command (tested in `test/canvas-polish.test.ts`). */
export const RULES: Rule[] = [
  { match: new RegExp(`^(?:unite|assemble|put together)\\s+${APP}$`, 'i'), run: slash('unite') },
  { match: new RegExp(`^(?:break apart|break up|split(?: up)?|take apart)\\s+${APP}$`, 'i'), run: slash('split') },
  { match: /^(?:(?:take|add|make|write|new)\s+(?:a\s+)?note|note)\s*[:,–-]\s*([\s\S]+)$/i, run: slash('note') },
  { match: /^(?:take|add|make|write)\s+a\s+note\s+(?:that\s+|to\s+)?([\s\S]+)$/i, run: slash('note') },
  { match: /^(?:(?:take|add|make|write)\s+a\s+note|new\s+note)$()/i, run: slash('note') },
  { match: /^(?:(?:show|open|add|get)\s+(?:me\s+)?)?(?:the\s+|a\s+)?weather(?:\s+(?:in|for|at)\s+(.+?))?\??$/i, run: slash('weather') },
  { match: /^what(?:'s| is) the weather(?:\s+(?:like\s+)?(?:in|for|at)\s+(.+?))?\??$/i, run: slash('weather') },
  { match: /^(?:(?:open|show|add|give me)\s+)?(?:a\s+|the\s+)?calculator$()/i, run: slash('calculator') },
  { match: /^(?:(?:show|open|add)\s+)?(?:a\s+|the\s+)?clock(?:\s+(?:in|for)\s+(.+?))?$/i, run: slash('clock') },
  { match: /^what time is it(?:\s+in\s+(.+?))?\??$/i, run: slash('clock') },
  { match: /^(?:arrange|tidy(?: up)?|clean up|organi[sz]e)\s+(?:the\s+|my\s+)?(?:canvas|board|cards)$()/i, run: slash('arrange') },
  { match: /^(?:new|create|make)\s+(?:a\s+)?folder(?:\s+(?:called|named))?\s+(.+)$/i, run: (found, report) => runSlash(`/folder new ${found[1]?.trim() ?? ''}`, report) },
  { match: /^(?:show|open)\s+(?:my\s+)?folders$()/i, run: slash('folder') },
  { match: /^(?:show|list)\s+(?:all\s+)?(?:my\s+)?(?:apps|commands)$()/i, run: slash('apps') },
  { match: /^(?:show|bring|add|open)\s+card\s+(\d+(?:\.\d+)?)$/i, run: slash('card') },
  { match: /^(?:open|show)\s+(?:the\s+|my\s+)?library$/i, run: (_found, report) => { openLibrary(); report('Your library is open: click a card to bring it onto the canvas.'); } },
  { match: /^(show|hide|turn on|turn off)\s+(?:the\s+)?timings?$/i, run: (found, report) => report(toggleTimings(/^(show|turn on)$/i.test(found[1] ?? ''))) },
];

/** The rule a sentence matches, if any (exported for the tests). */
export function matchSentence(text: string): { rule: Rule; found: RegExpMatchArray } | null {
  const sentence = text.trim().replace(/[.!]+$/, '');
  if (!sentence || sentence.startsWith('/') || sentence.length > 200) return null;
  for (const rule of RULES) {
    const found = sentence.match(rule.match);
    if (found) return { rule, found };
  }
  return null;
}

/** Runs a sentence that names a canvas action; false when it is not one. */
export async function routeSentence(text: string, report: Report): Promise<boolean> {
  const matched = matchSentence(text);
  if (!matched) return false;
  await matched.rule.run(matched.found, report);
  const input = document.getElementById('intent-input') as HTMLInputElement | null;
  if (input) input.value = '';
  return true;
}
