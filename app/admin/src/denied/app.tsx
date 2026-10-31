import { h, mount } from '@getforma/core';

// ADR-006 neutral denial: a verified identity with no current membership,
// entitlement or live pending admission. Sign-out only.
type DeniedProps = { csrfSignout: string };

function Denied(props: DeniedProps) {
  return (
    <main class="page">
      <a class="forma-brand" href="/" aria-label="Forma home"><span class="forma-mark-tile" aria-hidden="true"><span class="forma-mark"></span></span><span>forma<span class="forma-dot">.</span></span><span class="forma-product">canvas</span></a>
      <h1>No access</h1>
      <p class="lede">
        This account has no current access here. Sign out to continue with another account.
      </p>
      <form method="post" action="/auth/signout">
        <input type="hidden" name="csrf" value={props.csrfSignout} />
        <button type="submit">Sign out</button>
      </form>
    </main>
  );
}

mount(() => Denied({ csrfSignout: '' }), '#app');
