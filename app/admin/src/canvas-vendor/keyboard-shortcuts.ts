export interface KeyboardShortcutDefinition {
  id: string;
  group: "Canvas" | "Widget" | "Move" | "Global";
  keys: string;
  description: string;
}

export const KEYBOARD_SHORTCUTS: readonly KeyboardShortcutDefinition[] = [
  {
    id: "canvas-pan",
    group: "Canvas",
    keys: "Arrow keys · Shift for farther",
    description: "Pan while the workspace itself is focused.",
  },
  {
    id: "object-navigate",
    group: "Widget",
    keys: "Arrow keys · Home · End",
    description: "Move selection between widgets while a widget shell is focused.",
  },
  {
    id: "enter-widget",
    group: "Widget",
    keys: "Enter or F2",
    description: "Enter the selected widget's interactive content.",
  },
  {
    id: "move-rail",
    group: "Widget",
    keys: "M",
    description: "Focus the selected widget's Move rail.",
  },
  {
    id: "open-selected-controls",
    group: "Widget",
    keys: "⌘/Ctrl + Enter",
    description: "Open the selected widget controls in the Workspace Dock.",
  },
  {
    id: "nudge",
    group: "Move",
    keys: "Arrow keys",
    description: "Move the widget by 16 world pixels while its Move rail is focused.",
  },
  {
    id: "nudge-fast",
    group: "Move",
    keys: "Shift + Arrow keys",
    description: "Move the widget by 64 world pixels.",
  },
  {
    id: "neighbor-from-rail",
    group: "Move",
    keys: "⌘/Ctrl + Arrow keys",
    description: "Leave Move and select the geometric neighbor.",
  },
  {
    id: "composer",
    group: "Global",
    keys: "⌘/Ctrl + K",
    description: "Focus the workspace command composer without changing its draft.",
  },
  {
    id: "slash-commands",
    group: "Global",
    keys: "/",
    description: "Start a slash command while the canvas or a widget shell is focused.",
  },
  {
    id: "help",
    group: "Global",
    keys: "⌘/Ctrl + ?",
    description: "Open or close this keyboard shortcuts guide.",
  },
  {
    id: "escape",
    group: "Global",
    keys: "Escape",
    description: "Return one level, close the current layer, or clear selection.",
  },
] as const;

const TEXT_ENTRY_SELECTOR = [
  "input",
  "textarea",
  "select",
  "[contenteditable]:not([contenteditable='false'])",
  "[role='textbox']",
  "[role='searchbox']",
].join(",");

const INTERACTIVE_SELECTOR = [
  TEXT_ENTRY_SELECTOR,
  "button",
  "a[href]",
  "label",
  "summary",
  "[role='button']",
  "[role='checkbox']",
  "[role='combobox']",
  "[role='link']",
  "[role='menuitem']",
  "[role='option']",
  "[role='radio']",
  "[role='slider']",
  "[role='spinbutton']",
  "[role='switch']",
  "[role='tab']",
].join(",");

function composedPathMatches(event: Event, selector: string): boolean {
  return event.composedPath().some((candidate) => {
    const matches = (candidate as { matches?: unknown }).matches;
    return typeof matches === "function" &&
      (matches as (selector: string) => boolean).call(candidate, selector);
  });
}

export function eventOriginatesInTextEntry(event: Event): boolean {
  return composedPathMatches(event, TEXT_ENTRY_SELECTOR);
}

export function eventOriginatesInInteractiveControl(event: Event): boolean {
  return composedPathMatches(event, INTERACTIVE_SELECTOR);
}

export function hasPrimaryShortcutModifier(event: KeyboardEvent): boolean {
  return (event.metaKey || event.ctrlKey) && !event.altKey;
}

export function isKeyboardShortcutsChord(event: KeyboardEvent): boolean {
  return hasPrimaryShortcutModifier(event) && event.code === "Slash";
}
