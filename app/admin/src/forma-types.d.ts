import '@getforma/core';
// Published 3.2 declares JSX.Element as HTMLElement | DocumentFragment, while
// the positional list callback declaration is narrower. The native compiler
// separately proves each row callback returns one Element (BUILD-613). This
// consumer-only declaration (as spikes/APP-003) adds no runtime code.
declare module '@getforma/core' {
  function createList<T>(items: () => T[], key: (item: T) => string | number,
    render: (item: T, index: () => number) => JSX.Element): DocumentFragment;
}
// SVG <title> (a chart bar's exact reading) is admitted by the native
// compiler ("SVG title remains valid"); 3.2's JSX table omits the tag.
declare global {
  namespace JSX {
    interface IntrinsicElements {
      title: SVGAttributes;
    }
  }
}
