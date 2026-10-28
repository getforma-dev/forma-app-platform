import { h, mount } from '@getforma/core';

function About() {
  return (
    <main class="page">
      <h1>About this app</h1>
      <p class="lede">
        This page is rendered by Rust. It has no interactive regions, so its
        server-rendered build sends no JavaScript.
      </p>
      <p><a href="/">Back to the platform</a></p>
    </main>
  );
}

// The native build uses About as its empty-props root and publishes no JS.
mount(() => About(), '#app');
