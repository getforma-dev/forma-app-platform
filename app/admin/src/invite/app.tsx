import { h, mount } from '@getforma/core';

// APP-074 (ADR-006 GET /invite): the public invitation entry. One labeled
// code field posts to /invite/validate (same origin, no JavaScript). Every
// unusable code gets the same neutral refusal, so the page never says whether
// a code exists or for which organization; the submitted code is never
// rendered back, and a code is never read from a URL.
// `describedBy` names the element that explains the field: the refusal when
// one is shown, else the help text (computed by the handler).
type InviteProps = { failed: boolean; limited: boolean; describedBy: string };

function Invite(props: InviteProps) {
  return (
    <main class="page">
      <h1>Enter your invitation code</h1>
      <p class="lede">
        The beta is by invitation only. Your code opens a sign-up for the
        organization it was issued for; it cannot be used for any other.
      </p>
      {props.failed ? (
        <p class="notice" role="alert" id="invite-error">
          This code could not be used. It may be mistyped, expired, revoked or
          already used. Check it and try again, or ask the person who invited
          you for a new code.
        </p>
      ) : null}
      {props.limited ? (
        <p class="notice" role="alert" id="invite-error">
          Too many attempts from your network. Wait a few minutes, then try again.
        </p>
      ) : null}
      <form method="post" action="/invite/validate" class="entry">
        <label for="invite-code">Invitation code</label>
        <input
          id="invite-code"
          name="code"
          type="text"
          required
          autofocus
          autocomplete="off"
          maxLength={128}
          aria-describedby={props.describedBy}
        />
        <p class="help" id="invite-help">
          Dashes and spaces are ignored. After this step you create your account
          with the email address the invitation names.
        </p>
        <button type="submit">Continue</button>
      </form>
      <p>
        Already have an account? Use your organization's sign-in link, or <a href="/signin">sign in</a>.
      </p>
      <p><a href="/">Back to the platform</a></p>
    </main>
  );
}

// The native build uses Invite as its typed root and publishes no JS.
mount(() => Invite({ failed: false, limited: false, describedBy: 'invite-help' }), '#app');
