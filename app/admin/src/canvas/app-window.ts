import { el, originKind } from './render';

type Icon = 'app' | 'close' | 'open' | 'external' | 'settings' | 'sources' | 'saved' | 'summary' | 'ask' | 'share' | 'focus';
const paths: Record<Icon, string> = {
  app: 'M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z',
  close: 'm7 7 10 10M17 7 7 17',
  open: 'M8 4H4v4m12-4h4v4M4 16v4h4m12-4v4h-4',
  external: 'M14 4h6v6m0-6L10 14M10 4H4v16h16v-6',
  settings: 'M10 3h4l.5 2.5 1.7 1 2.4-.8 2 3.5-1.9 1.7v2.2l1.9 1.7-2 3.5-2.4-.8-1.7 1L14 21h-4l-.5-2.5-1.7-1-2.4.8-2-3.5 1.9-1.7v-2.2L3.4 9.2l2-3.5 2.4.8 1.7-1L10 3Z M8.5 12a3.5 3.5 0 1 0 7 0a3.5 3.5 0 1 0-7 0',
  sources: 'M4 3h10l6 6v12H4zM14 3v6h6M8 13h8M8 17h5',
  saved: 'M5 3h14v18l-7-4-7 4V3Z',
  summary: 'M5 3h14v18H5zM9 8h6M9 12h6M9 16h4',
  ask: 'M4 4h16v12H9l-5 4V4Z M8 8h8M8 12h5',
  share: 'M12 16V3m-4 4 4-4 4 4M5 12v9h14v-9',
  focus: 'M9 3H3v6m12-6h6v6M3 15v6h6m12-6v6h-6',
};

/** Fixed, trusted vector icons; app/model content is never parsed as markup. */
export function appWindowIcon(document_: Document, name: Icon): SVGSVGElement {
  const svg = document_.createElementNS('http://www.w3.org/2000/svg', 'svg');
  for (const [key, value] of Object.entries({viewBox: '0 0 24 24', width: '16', height: '16', fill: 'none', stroke: 'currentColor', 'stroke-width': '1.6', 'stroke-linecap': 'round', 'stroke-linejoin': 'round', 'aria-hidden': 'true', focusable: 'false'})) svg.setAttribute(key, value);
  const path = document_.createElementNS('http://www.w3.org/2000/svg', 'path');
  path.setAttribute('d', paths[name]); svg.append(path); return svg;
}

export function buildAppWindow(document_: Document, app: string, work: string, title: string, origins: string[], canShare: boolean, canEdit: boolean, presentation: string, standalone: boolean): HTMLElement {
  const iconButton = (icon: Icon, label: string, attributes: Record<string, string> = {}) => el(document_, 'button', {
    type: 'button', class: 'cv-window-tool', title: label, 'aria-label': label, ...attributes,
  }, appWindowIcon(document_, icon));
  const traffic = el(document_, 'div', {class: 'cv-window-controls', role: 'group', 'aria-label': 'Window controls'},
    iconButton('close', 'Remove from canvas', {class: 'cv-window-control is-close', 'data-app-view-remove': ''}));
  const picker = el(document_, 'div', {class: 'cv-app-view-picker', role: 'group', 'aria-label': 'App view'},
    ...(['face', 'stack', 'open'] as const).map(mode => el(document_, 'button', {type: 'button', 'data-app-view-mode': mode, 'aria-pressed': 'false'}, mode === 'face' ? 'Card' : mode === 'stack' ? 'Browse' : 'Open')));
  picker.hidden = standalone;
  const head = el(document_, 'header', {class: 'cv-unite-head', 'data-canvas-drag-handle': ''}, traffic,
    el(document_, 'div', {class: 'cv-unite-heading'},
      el(document_, 'span', {class: 'cv-unite-mark', 'aria-hidden': 'true'}, appWindowIcon(document_, 'app')),
      el(document_, 'h2', {class: 'cv-unite-title', title}, title)),
    el(document_, 'div', {class: 'cv-window-utilities'},
      iconButton('focus', 'Focus app · F', {'data-unite-focus': ''}),
      ...(canEdit ? [iconButton('settings', standalone ? 'Card settings' : 'App settings', {'data-edit-presentation': '', 'data-app': app, 'data-work': work})] : []),
      el(document_, 'a', {class: 'cv-window-tool', href: `/w/${app}/${work}`, target: '_blank', rel: 'noopener', title: 'Open in a new tab', 'aria-label': 'Open in a new tab'}, appWindowIcon(document_, 'external'))));
  const summary = el(document_, 'button', {type: 'button', class: 'cv-window-summary-toggle', 'data-app-summary': '', 'data-app-assistant': 'summary', 'aria-expanded': 'false', 'aria-controls': `app-assistant-${work}`, title: 'Show app summary'}, appWindowIcon(document_, 'summary'), el(document_, 'span', {}, 'Summary'));
  summary.hidden = standalone;
  const toolbar = el(document_, 'div', {class: 'cv-app-toolbar'}, picker,
    el(document_, 'div', {class: 'cv-unite-actions', role: 'group', 'aria-label': standalone ? 'Card' : 'App'}, summary,
      iconButton('sources', 'Source files', {'data-app-assistant': 'sources', 'aria-expanded': 'false', 'aria-controls': `app-assistant-${work}`}),
      iconButton('ask', standalone ? 'Ask about this card' : 'Ask about this app', {'data-app-assistant': 'ask', 'aria-expanded': 'false', 'aria-controls': `app-assistant-${work}`}),
      iconButton('saved', 'Saved answers', {'data-app-assistant': 'saved', 'aria-expanded': 'false', 'aria-controls': `app-assistant-${work}`}),
      ...(canShare ? [iconButton('share', 'Share app', {'data-unite-share': ''})] : [])));
  const window = el(document_, 'section', {class: 'cv-app-window', 'aria-label': `${title} window`}, head, toolbar,
    el(document_, 'div', {class: 'cv-app-content'},
      el(document_, 'div', {class: 'cv-app-face', 'data-app-view-face': ''}),
      el(document_, 'div', {class: 'cv-app-stack', 'data-app-view-stack': ''},
        el(document_, 'div', {class: 'cv-app-stack-nav', role: 'group', 'aria-label': 'Browse components'},
          el(document_, 'button', {type: 'button', class: 'cv-window-tool', 'data-app-stack-step': '-1', 'aria-label': 'Previous component'}, '‹'),
          el(document_, 'span', {'data-app-stack-position': '', 'aria-live': 'polite'}),
          el(document_, 'button', {type: 'button', class: 'cv-window-tool', 'data-app-stack-step': '1', 'aria-label': 'Next component'}, '›')),
        el(document_, 'div', {'data-app-stack-holder': ''})),
      el(document_, 'div', {'data-app-view-open': ''},
        el(document_, 'div', {'data-app-presentation-header': ''}),
        el(document_, 'div', {class: 'cv-unite-grid', 'data-presentation-layout': ''}))),
    el(document_, 'footer', {class: 'cv-app-statusbar'},
      el(document_, 'span', {class: 'cv-unite-count', 'data-unite-count': ''}),
      el(document_, 'p', {class: 'cv-origins cv-unite-origins', 'aria-label': 'How this was assembled', title: origins.join(' · ')},
        ...origins.map(text => el(document_, 'span', {class: 'cv-origin', 'data-origin': originKind(text)}, text)))));
  const root = el(document_, 'article', {class: 'cv-unite', 'data-unite': work, 'data-kind': 'unite', 'data-presentation': presentation, 'data-app': app, 'data-work': work, 'aria-label': title},
    window, el(document_, 'aside', {id: `app-assistant-${work}`, class: 'cv-app-companion', 'data-app-companion': '', hidden: '', 'aria-label': `Assistant for ${title}`}),
    el(document_, 'div', {'data-app-view-parking': ''}),
    el(document_, 'button', {type: 'button', class: 'cv-unite-resize', 'data-unite-resize': '', 'aria-label': 'Resize the app', title: 'Drag to resize this app'}));
  root.dataset.placementKind = standalone ? 'card' : 'app';
  const parking = root.querySelector<HTMLElement>('[data-app-view-parking]')!; parking.hidden = true; parking.inert = true;
  root.querySelector<HTMLElement>('[data-app-companion]')!.hidden = true;
  return root;
}
