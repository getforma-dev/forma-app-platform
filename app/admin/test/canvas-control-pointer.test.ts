import assert from 'node:assert/strict';
import { test } from 'node:test';
import { preserveCanvasControlPointer } from '../src/canvas/controls.ts';

function press(kind: string, { button = 0, pan = false, inCard = true, inHost = true, prevented = false } = {}) {
  const interactive = ['button', 'input', 'select', 'textarea', 'label', 'summary', 'link'].includes(kind);
  const control = { matches: (selector: string) => interactive && selector.includes(kind === 'link' ? 'a[href]' : kind) };
  // The original target may be an SVG path inside an actual button.
  const target = { closest: () => inCard ? {} : null };
  const host = { contains: () => inHost, querySelector: () => pan ? {} : null } as unknown as HTMLElement;
  const event = { target, button, defaultPrevented: prevented, stopped: false,
    composedPath: () => [target, control, host], stopPropagation() { this.stopped = true; },
    preventDefault() { this.defaultPrevented = true; } };
  return { event, host, guard: () => preserveCanvasControlPointer(event as unknown as PointerEvent, host) };
}

test('inactive card controls retain the original click target without suppressing native focus or submission', () => {
  for (const kind of ['button', 'input', 'select', 'textarea', 'label', 'summary', 'link']) {
    const { event, guard } = press(kind);
    let selectedDuringPress = false;
    assert.equal(guard(), true, kind);
    if (!event.stopped) selectedDuringPress = true;
    assert.equal(selectedDuringPress, false, `${kind}: canvas cannot reflow the target before click`);
    assert.equal(event.defaultPrevented, false, `${kind}: native focus/activation remains enabled`);
  }
});

test('header, empty-space, outside controls, middle click and explicit pan retain canvas handling', () => {
  for (const fixture of [press('header'), press('button', { inCard: false }), press('button', { inHost: false }),
    press('button', { button: 1 }), press('button', { pan: true }), press('button', { prevented: true })]) {
    assert.equal(fixture.guard(), false);
    assert.equal(fixture.event.stopped, false);
  }
});
