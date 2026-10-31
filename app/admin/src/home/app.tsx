import { h, mount } from '@getforma/core';

// APP-138: the public entry is a native product page, with no demo counter
// or browser island. Invitation and organization sign-in remain unchanged.
function Home() {
  return (
    <main class="page home-page">
      <a class="forma-brand" href="/" aria-label="Forma home"><span class="forma-mark-tile" aria-hidden="true"><span class="forma-mark"></span></span><span>forma<span class="forma-dot">.</span></span><span class="forma-product">canvas</span></a>
      <p class="page-eyebrow">A workspace built around you</p>
      <h1>Your work.<br />In good shape.</h1>
      <p class="lede">Turn an idea into an app. Bring your data, shape your tools, and work together on one canvas.</p>
      <nav aria-label="Beta access" class="entry-links">
        <a href="/invite">Enter an invitation code</a>
        <a href="/signin">Sign in <span aria-hidden="true">↗</span></a>
      </nav>
      <p class="help">Forma Canvas is in private beta. Use your invitation code or your organization’s sign-in link to get started.</p>
      <div class="home-principles" aria-label="How Forma works">
        <div><span>01 / CREATE</span><h2>Start with an idea.</h2><p>Describe the app you need, then shape it as you work.</p></div>
        <div><span>02 / UNDERSTAND</span><h2>Follow the data.</h2><p>Explore your numbers with receipts you can inspect.</p></div>
        <div><span>03 / ACT</span><h2>Stay in control.</h2><p>Review and confirm changes before they happen.</p></div>
      </div>
      <p class="home-footer">Built on Forma Stack. <a href="/about">Meet Forma <span aria-hidden="true">↗</span></a></p>
    </main>
  );
}

mount(() => Home(), '#app');
