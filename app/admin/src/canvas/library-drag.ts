// APP-104 / whole-app views: drag an explicitly standalone whole work from Cards. A desktop pointer (mouse
// or pen) that moves past a short threshold picks the card up. Touch never
// starts a drag, so the library list keeps scrolling. Dropping on the canvas
// places that whole work through the atomic view controller, at the
// world point under the pointer (the stage is `translate(pan) scale(zoom)`
// with its origin at 0,0). A drop on the library, the dock or anywhere else,
// or Escape, places nothing. A card already on the board is not placed again.
//
// The gesture math is separate from the DOM so the tests can name a pan, a
// zoom and a miss without a browser.

export const DRAG_THRESHOLD_PX = 6;
/** Screen pixels from the card's top-left to the pointer, so the card sits under the cursor. */
export const DROP_INSET_PX = 24;

export type ClientPoint = { x: number; y: number };
export type Camera = { panX: number; panY: number; zoom: number };
export type Box = { left: number; top: number; right: number; bottom: number };
export type DragCard = {
  id: string;
  title: string;
  number: string;
  kind: string;
  app: string;
  work: string;
};

export type DragFrame = {
  pointerId: number;
  x: number;
  y: number;
  /** The pointer is over the canvas surface, not the library, dock or top bar. */
  accepting: boolean;
  viewport: Box | null;
  camera: Camera | null;
};

export type DragEffect =
  | { type: 'ignore' }
  | { type: 'pending' }
  | { type: 'click' }
  | { type: 'cancel' }
  | { type: 'dragging'; drop: 'canvas' | 'outside' | 'placed'; origin: { x: number; y: number } | null; card: DragCard }
  | { type: 'place'; card: DragCard; at: { x: number; y: number } };

const finite = (value: number) => typeof value === 'number' && Number.isFinite(value);

/** World point under a client point. Null when the camera cannot be applied. */
export function worldPoint(client: ClientPoint, viewport: Box, camera: Camera): { x: number; y: number } | null {
  if (!finite(camera.zoom) || camera.zoom <= 0 || !finite(camera.panX) || !finite(camera.panY)) return null;
  if (![client.x, client.y, viewport.left, viewport.top].every(finite)) return null;
  return {
    x: (client.x - viewport.left - camera.panX) / camera.zoom,
    y: (client.y - viewport.top - camera.panY) / camera.zoom,
  };
}

/** Card top-left for a drop: the pointer stays `inset` screen pixels inside the card. */
export function dropOrigin(world: { x: number; y: number }, camera: Camera, inset = DROP_INSET_PX): { x: number; y: number } {
  const shift = inset / camera.zoom;
  return { x: Math.round(world.x - shift), y: Math.round(world.y - shift) };
}

export function pointerOver(client: ClientPoint, box: Box): boolean {
  return client.x >= box.left && client.x <= box.right && client.y >= box.top && client.y <= box.bottom;
}

/**
 * A drop lands when the pointer is inside the canvas viewport and not inside
 * a blocker (the open library, the dock, the top bar).
 */
export function acceptsDrop(client: ClientPoint, viewport: Box | null, blockers: Box[]): boolean {
  if (!viewport || !pointerOver(client, viewport)) return false;
  return !blockers.some((box) => pointerOver(client, box));
}

/** Where the drop footprint is drawn, in client pixels. Its top-left is the card's top-left. */
export function ghostScreen(
  origin: { x: number; y: number },
  viewport: Box,
  camera: Camera,
  footprint: { width: number; height: number },
): { left: number; top: number; width: number; height: number } {
  return {
    left: viewport.left + origin.x * camera.zoom + camera.panX,
    top: viewport.top + origin.y * camera.zoom + camera.panY,
    width: footprint.width * camera.zoom,
    height: footprint.height * camera.zoom,
  };
}

type Down = {
  pointerType: string;
  button: number;
  pointerId: number;
  x: number;
  y: number;
  card: DragCard | null;
  /** The press is on the naming control, which keeps its click. */
  editing: boolean;
  onBoard: boolean;
};

export function createDragController() {
  let phase: 'idle' | 'pending' | 'dragging' = 'idle';
  let card: DragCard | null = null;
  let from: ClientPoint | null = null;
  let pointerId = -1;
  let onBoard = false;

  function reset(): void {
    phase = 'idle';
    card = null;
    from = null;
    pointerId = -1;
    onBoard = false;
  }

  function view(frame: DragFrame): DragEffect {
    if (!card) return { type: 'ignore' };
    if (onBoard) return { type: 'dragging', drop: 'placed', origin: null, card };
    if (!frame.accepting || !frame.viewport || !frame.camera) return { type: 'dragging', drop: 'outside', origin: null, card };
    const world = worldPoint(frame, frame.viewport, frame.camera);
    if (!world) return { type: 'dragging', drop: 'outside', origin: null, card };
    return { type: 'dragging', drop: 'canvas', origin: dropOrigin(world, frame.camera), card };
  }

  return {
    phase: () => phase,
    down(input: Down): DragEffect {
      if (phase !== 'idle') return { type: 'ignore' };
      // Touch scrolls the library. Only a mouse or a pen picks a card up.
      if (input.pointerType !== 'mouse' && input.pointerType !== 'pen') return { type: 'ignore' };
      if (input.button !== 0 || input.editing || !input.card) return { type: 'ignore' };
      phase = 'pending';
      card = input.card;
      from = { x: input.x, y: input.y };
      pointerId = input.pointerId;
      onBoard = input.onBoard;
      return { type: 'pending' };
    },
    move(frame: DragFrame): DragEffect {
      if (phase === 'idle' || frame.pointerId !== pointerId || !from || !card) return { type: 'ignore' };
      if (phase === 'pending' && Math.hypot(frame.x - from.x, frame.y - from.y) < DRAG_THRESHOLD_PX) return { type: 'pending' };
      phase = 'dragging';
      return view(frame);
    },
    up(frame: DragFrame): DragEffect {
      if (phase === 'idle' || frame.pointerId !== pointerId || !card) return { type: 'ignore' };
      if (phase === 'pending') {
        reset();
        return { type: 'click' };
      }
      const shown = view(frame);
      const held = card;
      reset();
      if (shown.type === 'dragging' && shown.drop === 'canvas' && shown.origin) {
        return { type: 'place', card: held, at: shown.origin };
      }
      return { type: 'cancel' };
    },
    cancel(): DragEffect {
      if (phase === 'idle') return { type: 'ignore' };
      reset();
      return { type: 'cancel' };
    },
  };
}

export type LibraryDragHooks = {
  cardFrom(row: HTMLElement): DragCard | null;
  onBoard(id: string): boolean;
  place(card: DragCard, at: { x: number; y: number }): void;
  camera(): Camera | null;
  viewport(): HTMLElement | null;
  footprint(kind: string): { width: number; height: number };
};

function boxOf(node: HTMLElement): Box {
  const rect = node.getBoundingClientRect();
  return { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom };
}

/**
 * Wires the gesture on `document_`. A drag past the threshold captures the
 * pointer and swallows the click that would otherwise Add or Remove. A press
 * that does not move leaves that click alone.
 */
export function bindLibraryDrag(document_: Document, hooks: LibraryDragHooks): void {
  const controller = createDragController();
  let swallowClick = false;
  // Set once the press is a drag (or Escape cancelled it) so the button's
  // click does not also Add or Remove. Cleared on the next turn if no click
  // follows, so a later click still works.
  let suppressClick = false;
  let captured: HTMLElement | null = null;
  let preview: HTMLElement | null = null;
  let ghost: HTMLElement | null = null;
  let source: HTMLElement | null = null;

  const clear = () => {
    preview?.remove();
    ghost?.remove();
    preview = null;
    ghost = null;
    source?.classList.remove('is-dragging');
    source = null;
    document_.body.classList.remove('is-library-dragging');
    document_.getElementById('cv-canvas')?.removeAttribute('data-library-drop');
    if (captured) {
      const id = Number(captured.dataset.captureId);
      if (Number.isFinite(id)) {
        try { captured.releasePointerCapture(id); } catch { /* the pointer is already gone */ }
      }
      delete captured.dataset.captureId;
      captured = null;
    }
  };

  const frameFor = (event: PointerEvent): DragFrame => {
    const viewportEl = hooks.viewport();
    const camera = hooks.camera();
    const viewport = viewportEl ? boxOf(viewportEl) : null;
    const client = { x: event.clientX, y: event.clientY };
    // The empty welcome sits in `#cv-canvas`, over the viewport, so a hit
    // there is still the canvas. The library, dock and top bar are not.
    const hit = document_.elementFromPoint(event.clientX, event.clientY);
    const canvas = document_.getElementById('cv-canvas');
    const onCanvas = !!hit && !hit.closest('#cv-library, .cv-dock, .cv-top')
      && (!!canvas?.contains(hit) || !!viewportEl?.contains(hit));
    const accepting = !!viewport && pointerOver(client, viewport) && onCanvas;
    return { pointerId: event.pointerId, x: event.clientX, y: event.clientY, accepting, viewport, camera };
  };

  const paint = (effect: DragEffect, event: PointerEvent) => {
    if (effect.type !== 'dragging' || !preview) return;
    preview.dataset.drop = effect.drop;
    preview.style.left = `${event.clientX + 14}px`;
    preview.style.top = `${event.clientY + 14}px`;
    const hint = preview.querySelector('.cv-lib-drag-hint');
    if (hint) {
      hint.textContent = effect.drop === 'canvas' ? 'Drop to place' : effect.drop === 'placed' ? 'Already on the canvas' : 'Drop on the canvas';
    }
    const canvas = document_.getElementById('cv-canvas');
    if (effect.drop === 'canvas') canvas?.setAttribute('data-library-drop', 'true');
    else canvas?.removeAttribute('data-library-drop');
    if (effect.drop === 'canvas' && effect.origin && ghost) {
      const viewport = hooks.viewport();
      const camera = hooks.camera();
      if (viewport && camera) {
        const screen = ghostScreen(effect.origin, boxOf(viewport), camera, hooks.footprint(effect.card.kind));
        ghost.hidden = false;
        ghost.style.left = `${screen.left}px`;
        ghost.style.top = `${screen.top}px`;
        ghost.style.width = `${screen.width}px`;
        ghost.style.height = `${screen.height}px`;
      }
    } else if (ghost) ghost.hidden = true;
  };

  const show = (card: DragCard, row: HTMLElement) => {
    source = row;
    row.classList.add('is-dragging');
    document_.body.classList.add('is-library-dragging');
    preview = document_.createElement('div');
    preview.className = 'cv-lib-drag';
    preview.setAttribute('aria-hidden', 'true');
    preview.style.pointerEvents = 'none';
    preview.dataset.drop = 'outside';
    const number = document_.createElement('span');
    number.className = 'cv-lib-num';
    number.textContent = card.number;
    const title = document_.createElement('span');
    title.className = 'cv-lib-drag-title';
    title.textContent = card.title;
    const hint = document_.createElement('span');
    hint.className = 'cv-lib-drag-hint';
    hint.textContent = 'Drop on the canvas';
    preview.append(number, title, hint);
    ghost = document_.createElement('div');
    ghost.className = 'cv-lib-drop';
    ghost.style.pointerEvents = 'none';
    ghost.hidden = true;
    document_.body.append(preview, ghost);
  };

  document_.addEventListener('pointerdown', (event) => {
    if (event.pointerType === 'touch') return;
    const target = event.target as Element | null;
    const row = target?.closest<HTMLElement>('[data-lib-entry-drag]');
    if (!row || !row.closest('#cv-library')) return;
    const editing = !!target?.closest('button, a, summary, details, input, select, textarea, [data-lib-edit]');
    const card = editing ? null : hooks.cardFrom(row);
    const effect = controller.down({
      pointerType: event.pointerType,
      button: event.button,
      pointerId: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      card,
      editing,
      onBoard: card ? hooks.onBoard(card.id) : false,
    });
    if (effect.type === 'pending' && card) captured = row;
  });

  document_.addEventListener('pointermove', (event) => {
    if (controller.phase() === 'idle') return;
    const before = controller.phase();
    const effect = controller.move(frameFor(event));
    if (effect.type !== 'dragging') return;
    if (before !== 'dragging' && captured) {
      suppressClick = true;
      try {
        captured.setPointerCapture(event.pointerId);
        captured.dataset.captureId = String(event.pointerId);
      } catch {
        // capture is a convenience; the document listeners still follow the pointer
      }
      show(effect.card, captured);
    }
    event.preventDefault();
    paint(effect, event);
  });

  const armSwallow = () => {
    if (!suppressClick) return;
    suppressClick = false;
    swallowClick = true;
    document_.defaultView?.setTimeout(() => { swallowClick = false; }, 0);
  };

  const end = (event: PointerEvent, cancel: boolean) => {
    if (!cancel && controller.phase() === 'idle') {
      armSwallow();
      return;
    }
    const effect = cancel ? controller.cancel() : controller.up(frameFor(event));
    if (effect.type === 'ignore') return;
    if (effect.type === 'place' && !hooks.onBoard(effect.card.id)) hooks.place(effect.card, effect.at);
    if (effect.type !== 'click') suppressClick = true;
    armSwallow();
    clear();
  };

  document_.addEventListener('pointerup', (event) => end(event, false));
  document_.addEventListener('pointercancel', (event) => end(event, true));
  document_.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape' || controller.phase() === 'idle') return;
    event.preventDefault();
    event.stopPropagation();
    suppressClick = true;
    controller.cancel();
    clear();
  }, true);

  document_.addEventListener('click', (event) => {
    if (!swallowClick) return;
    swallowClick = false;
    event.preventDefault();
    event.stopPropagation();
  }, true);
}
