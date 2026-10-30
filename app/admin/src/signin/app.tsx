import { h, mount } from '@getforma/core';

// ADR-006 neutral sign-in page: one same-origin form when a well-formed
// organization handle is known (from the path or the routing hint). The page
// is identical for known and unknown handles; no lookup decides it.
type SigninProps = { action: string; hasAction: boolean; failed: boolean };

function Signin(props: SigninProps) {
  return (
    <main class="page">
      <h1>Sign in</h1>
      {props.failed ? (
        <p class="notice" role="alert">
          Sign-in could not continue. Start again from your organization's sign-in link.
        </p>
      ) : null}
      {props.hasAction ? (
        <form method="post" action={props.action}>
          <button type="submit">Continue to sign in</button>
        </form>
      ) : (
        <p class="lede">Open your organization's sign-in link to continue.</p>
      )}
      <p><a href="/">Back to the platform</a></p>
    </main>
  );
}

// The native build uses Signin as its typed root and publishes no JS.
mount(() => Signin({ action: '', hasAction: false, failed: false }), '#app');
