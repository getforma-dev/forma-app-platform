// APP-104: dragging one library card onto the canvas. The gesture math is
// pure: pan and zoom, a miss, a cancel, and a card that is already placed.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  acceptsDrop, bindLibraryDrag, createDragController, dropOrigin, DROP_INSET_PX, ghostScreen, worldPoint,
  type Camera, type DragCard,
} from '../src/canvas/library-drag.ts';

const card: DragCard = {
  id: 'c-work-spend', title: 'Spend by vendor', number: '3.2', kind: 'summary',
  app: '00000000-0000-4000-8000-000000001681', work: '00000000-0000-4000-8000-000000001682',
};
const viewport = { left: 10, top: 20, right: 900, bottom: 700 };
const camera: Camera = { panX: 80, panY: 40, zoom: 2 };
const library = { left: 560, top: 72, right: 960, bottom: 800 };

function down(drag: ReturnType<typeof createDragController>, extra: Partial<{ pointerType: string; button: number; pointerId: number; x: number; y: number; editing: boolean; onBoard: boolean; card: DragCard | null }> = {}) {
  return drag.down({
    pointerType: 'mouse', button: 0, pointerId: 1, x: 100, y: 120, card, editing: false, onBoard: false, ...extra,
  });
}

test('the drop point follows pan and zoom, and the footprint sits on that point', () => {
  // Pointer 24px inside a card whose top-left is world (300, 200).
  const pointer = {
    x: viewport.left + 300 * camera.zoom + camera.panX + DROP_INSET_PX,
    y: viewport.top + 200 * camera.zoom + camera.panY + DROP_INSET_PX,
  };
  const world = worldPoint(pointer, viewport, camera);
  assert.ok(world);
  assert.deepEqual(dropOrigin(world, camera), { x: 300, y: 200 });
  const ghost = ghostScreen({ x: 300, y: 200 }, viewport, camera, { width: 400, height: 250 });
  assert.equal(ghost.left, pointer.x - DROP_INSET_PX);
  assert.equal(ghost.top, pointer.y - DROP_INSET_PX);
  assert.equal(ghost.width, 800);
  assert.equal(ghost.height, 500);
  assert.equal(worldPoint(pointer, viewport, { ...camera, zoom: 0 }), null);
});

test('a drop on the library or off the canvas is not a placement', () => {
  assert.equal(acceptsDrop({ x: 200, y: 200 }, viewport, [library]), true);
  assert.equal(acceptsDrop({ x: 700, y: 200 }, viewport, [library]), false);
  assert.equal(acceptsDrop({ x: 2, y: 200 }, viewport, []), false);
  assert.equal(acceptsDrop({ x: 200, y: 200 }, null, []), false);
});

test('a desktop drag places the card, and a short press stays a click', () => {
  const drag = createDragController();
  assert.equal(down(drag).type, 'pending');
  assert.equal(drag.move({ pointerId: 1, x: 104, y: 122, accepting: true, viewport, camera }).type, 'pending');
  assert.equal(drag.up({ pointerId: 1, x: 104, y: 122, accepting: true, viewport, camera }).type, 'click');

  assert.equal(down(drag, { pointerId: 4, x: 80, y: 90 }).type, 'pending');
  const moved = drag.move({
    pointerId: 4,
    x: viewport.left + 300 * camera.zoom + camera.panX + DROP_INSET_PX,
    y: viewport.top + 200 * camera.zoom + camera.panY + DROP_INSET_PX,
    accepting: true, viewport, camera,
  });
  assert.equal(moved.type, 'dragging');
  if (moved.type === 'dragging') assert.equal(moved.drop, 'canvas');
  const placed = drag.up({
    pointerId: 4,
    x: viewport.left + 300 * camera.zoom + camera.panX + DROP_INSET_PX,
    y: viewport.top + 200 * camera.zoom + camera.panY + DROP_INSET_PX,
    accepting: true, viewport, camera,
  });
  assert.deepEqual(placed, { type: 'place', card, at: { x: 300, y: 200 } });
});

test('dropping outside, cancelling, or dragging a placed card does not place', () => {
  const outside = createDragController();
  down(outside, { pointerId: 2 });
  outside.move({ pointerId: 2, x: 40, y: 40, accepting: false, viewport, camera });
  assert.equal(outside.up({ pointerId: 2, x: 40, y: 40, accepting: false, viewport, camera }).type, 'cancel');

  const cancelled = createDragController();
  down(cancelled, { pointerId: 3, x: 50, y: 50 });
  cancelled.move({ pointerId: 3, x: 80, y: 90, accepting: true, viewport, camera });
  assert.equal(cancelled.cancel().type, 'cancel');
  assert.equal(cancelled.phase(), 'idle');
  assert.equal(cancelled.up({ pointerId: 3, x: 80, y: 90, accepting: true, viewport, camera }).type, 'ignore');
  assert.equal(down(cancelled, { pointerId: 8 }).type, 'pending', 'the next drag still starts');

  const placed = createDragController();
  down(placed, { pointerId: 5, onBoard: true });
  const over = placed.move({ pointerId: 5, x: 220, y: 240, accepting: true, viewport, camera });
  assert.equal(over.type, 'dragging');
  if (over.type === 'dragging') assert.equal(over.drop, 'placed');
  assert.equal(placed.up({ pointerId: 5, x: 220, y: 240, accepting: true, viewport, camera }).type, 'cancel');
});

test('touch and the naming control do not start a drag', () => {
  const drag = createDragController();
  assert.equal(down(drag, { pointerType: 'touch' }).type, 'ignore');
  assert.equal(drag.up({ pointerId: 1, x: 400, y: 400, accepting: true, viewport, camera }).type, 'ignore');
  assert.equal(down(drag, { pointerType: 'pen', pointerId: 6, editing: true }).type, 'ignore');
  assert.equal(down(drag, { pointerType: 'pen', pointerId: 7, button: 2 }).type, 'ignore');
  assert.equal(down(drag, { pointerId: 9, card: null }).type, 'ignore');
});


test('the DOM binding starts only from an explicit whole standalone entry, never app parts or controls', () => {
  const listeners = new Map<string, (event: unknown) => void>();
  const document_ = { addEventListener: (name: string, handler: (event: unknown) => void) => listeners.set(name, handler) };
  const seen: unknown[] = [];
  bindLibraryDrag(document_ as unknown as Document, {
    cardFrom: row => { seen.push(row); return card; }, onBoard: () => false,
    place: () => assert.fail('pointerdown must never place'), camera: () => camera,
    viewport: () => null, footprint: () => ({ width: 400, height: 250 }),
  });
  const row = { closest: (selector: string) => selector === '#cv-library' ? {} : null };
  const target = (entry: boolean, interactive: boolean) => ({
    closest: (selector: string) => selector === '[data-lib-entry-drag]' ? (entry ? row : null) : (interactive ? {} : null),
  });
  const downOn = (value: unknown) => listeners.get('pointerdown')!({ target: value, pointerType: 'mouse', button: 0, pointerId: 1, clientX: 10, clientY: 10 });
  downOn(target(false, false)); // App heading or hidden component metadata.
  downOn(target(true, true)); // Add, menu, alias, or naming control.
  assert.deepEqual(seen, []);
  downOn(target(true, false));
  assert.deepEqual(seen, [row]);
});
