import '@getforma/core';
// Published 3.2 declares JSX.Element as HTMLElement | DocumentFragment,
// while the positional list callback declaration is narrower. Native compilation
// and real-browser namespace/identity checks separately prove these Element roots.
// This consumer-only declaration adds no runtime code and changes no package.
declare module '@getforma/core' {
  function createList<T>(items: () => T[], key: (item: T) => string | number,
    render: (item: T, index: () => number) => JSX.Element): DocumentFragment;
}
declare global {
  namespace JSX { interface IntrinsicElements { title: SVGAttributes; } }
}
