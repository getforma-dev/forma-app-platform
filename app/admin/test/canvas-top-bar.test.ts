// APP-224: the canvas top bar's width tiers (src/styles/canvas.css). A
// Playwright pass measures the result (evidence/APP-223); this pins what
// each tier may hide, and that nothing above phone width hides the
// controls the bar must keep: Create app, Library, Help, the name menu and
// Sign out.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

type Rule = { media: string; selectors: string[]; body: string };

const css = readFileSync(new URL('../src/styles/canvas.css', import.meta.url), 'utf8');

/** Every style rule with the media query around it (one level, as the file uses). */
function rules(text: string): Rule[] {
  const out: Rule[] = [];
  const walk = (source: string, media: string) => {
    let at = 0;
    while (at < source.length) {
      const open = source.indexOf('{', at);
      if (open < 0) break;
      const prelude = source.slice(at, open).trim();
      if (prelude.startsWith('@')) {
        let depth = 1;
        let end = open + 1;
        while (depth > 0 && end < source.length) {
          if (source[end] === '{') depth += 1;
          else if (source[end] === '}') depth -= 1;
          end += 1;
        }
        if (prelude.startsWith('@media')) walk(source.slice(open + 1, end - 1), prelude);
        at = end;
      } else {
        const close = source.indexOf('}', open);
        out.push({ media, selectors: prelude.split(',').map((s) => s.trim()), body: source.slice(open + 1, close) });
        at = close + 1;
      }
    }
  };
  walk(text.replace(/\/\*[\s\S]*?\*\//g, ''), '');
  return out;
}

const hides = (rule: Rule) => /display:\s*none|visibility:\s*hidden|clip-path:\s*inset\(50%\)/.test(rule.body);

test('the top bar tiers hide only the collapsible items, and the name menu takes each moved link (APP-224)', () => {
  const start = css.indexOf('/* APP-224: the top bar fits');
  assert.ok(start >= 0, 'the APP-224 block');
  const tiers = rules(css.slice(start));
  assert.deepEqual(
    tiers.filter(hides).map((rule) => [rule.media, rule.selectors.join(', ')]),
    [
      ['@media (min-width: 761px) and (max-width: 1800px)', '.cv-live [data-live-text], .cv-connected-status'],
      ['@media (min-width: 761px) and (max-width: 1536px)', '.cv-demo-note'],
      ['@media (min-width: 761px) and (max-width: 1440px)', '.cv-avatar'],
      ['@media (min-width: 761px) and (max-width: 1366px)', '.cv-top-status > [data-nav="workspace"], .cv-top-status > [data-nav="admin"]'],
      ['@media (min-width: 761px) and (max-width: 1152px)', '.cv-top-status > [data-nav="shared"]'],
      ['@media (min-width: 761px) and (max-width: 900px)', '.cv-product'],
    ],
  );
  // The live state and the connected-apps line are only visually hidden:
  // their words stay for assistive technology.
  const quiet = tiers.find((rule) => rule.selectors.includes('.cv-connected-status'));
  assert.ok(quiet && /position:\s*absolute/.test(quiet.body) && !/display:\s*none/.test(quiet.body));
  // Every link the bar hides is shown in the name menu at the same widths.
  for (const rule of tiers.filter(hides)) {
    for (const selector of rule.selectors) {
      const nav = selector.match(/^\.cv-top-status > \[data-nav="(\w+)"\]$/);
      if (!nav) continue;
      const shown = tiers.find((other) => other.media === rule.media && /display:\s*block/.test(other.body)
        && other.selectors.includes(`.cv-name-nav [data-nav="${nav[1]}"]`));
      assert.ok(shown, `the name menu shows ${nav[1]} where the bar hides it`);
    }
  }
});

test('nothing above phone width hides the controls the bar keeps (APP-224)', () => {
  const keep = [/\[data-create-app\]/, /#cv-library-toggle/, /\.cv-lib-open(?![\w-])/, /\.cv-help-menu(?![\w-])/, /\.cv-name-menu(?![\w-])/, /\.cv-signout(?![\w-])/, /\.cv-top-status(?![\w-])/];
  const phoneOnly = (media: string) => /max-width:\s*(760|380)px/.test(media) && !/min-width/.test(media);
  for (const rule of rules(css).filter(hides)) {
    if (phoneOnly(rule.media)) continue;
    for (const selector of rule.selectors) {
      const target = selector.split(/\s*[>+~]\s*|\s+/).pop() ?? '';
      // The rule's own target, or a bare element inside a kept control.
      const hit = keep.some((pattern) => pattern.test(target) || (/^[a-z]+$/.test(target) && pattern.test(selector)));
      assert.ok(!hit, `${rule.media || 'every width'}: ${selector} hides a control the bar keeps`);
    }
  }
});
