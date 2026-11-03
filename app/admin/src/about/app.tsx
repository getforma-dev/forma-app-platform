import { h, mount } from '@getforma/core';

function About() {
  return (
    <main class="page">
      <a class="forma-brand" href="/" aria-label="Forma home"><span class="forma-mark-tile" aria-hidden="true"><span class="forma-mark"></span></span><span>forma<span class="forma-dot">.</span></span><span class="forma-product">canvas</span></a>
      <h1>Built on Forma.</h1>
      <p class="lede">
        Forma Canvas brings your apps, data and decisions into one workspace.
        Describe what you need, inspect the data behind your answers, and
        review changes before you confirm them.
      </p>
      <p>Part of the Forma family. <a href="https://getforma.dev">Explore Forma Stack ↗</a></p>
      <p><a href="/">Back to Forma Canvas</a></p>
    </main>
  );
}

// The native build uses About as its empty-props root and publishes no JS.
mount(() => About(), '#app');
