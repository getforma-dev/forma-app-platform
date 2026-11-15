import { h } from '@getforma/core';

// APP-223: the one site footer of the document-style pages. It is static: it
// takes no props and renders nothing from the request, so the uniform 404
// (ADR-006, APP-217) stays one answer whatever the path. No script. Each page
// places it after its <main>, so the <footer> is the page's contentinfo
// landmark. The full-screen canvas pages (/canvas, /shared) do not use it.
export function SiteFooter() {
  return (
    <footer class="site-footer">
      <nav class="site-footer-nav" aria-label="Footer">
        <div>
          <h2 class="site-footer-heading">Forma Canvas</h2>
          <ul>
            <li><a href="/">Home</a></li>
            <li><a href="/about">About</a></li>
            <li><a href="/signin">Sign in</a></li>
            <li><a href="/invite">Have an invitation?</a></li>
          </ul>
        </div>
        <div>
          <h2 class="site-footer-heading">Forma</h2>
          <ul>
            <li><a href="https://getforma.dev" rel="noopener">Forma Stack</a></li>
            <li><a href="https://getforma.dev/blog" rel="noopener">Blog</a></li>
            <li><a href="https://getforma.dev/labs/" rel="noopener">Server lab</a></li>
          </ul>
        </div>
        <div>
          <h2 class="site-footer-heading">Help &amp; legal</h2>
          <ul>
            <li><a href="mailto:support@getforma.dev">Contact</a></li>
            <li><a href="/notice">Beta terms &amp; privacy</a></li>
            <li><a href="/.well-known/security.txt">Security</a></li>
          </ul>
        </div>
      </nav>
      <p class="site-footer-bottom">
        <span class="site-footer-brand" aria-hidden="true"><span class="forma-mark-tile"><span class="forma-mark"></span></span><span>forma<span class="forma-dot">.</span></span></span>
        <span>© 2026 Forma · Forma Canvas is in private beta.</span>
      </p>
    </footer>
  );
}
