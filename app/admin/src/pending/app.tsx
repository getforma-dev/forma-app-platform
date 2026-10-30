import { h, mount } from '@getforma/core';

// ADR-006 restricted pending page: the identity is verified, the mailbox is
// not. It reads nothing from the workspace; a resend result is shown as a
// request, never as delivery.
type PendingProps = {
  csrfResend: string;
  csrfSignout: string;
  resendAvailable: boolean;
  cooldownSeconds: number;
  held: boolean;
  hoursRemaining: number;
  requested: boolean;
  unavailable: boolean;
};

function Pending(props: PendingProps) {
  return (
    <main class="page">
      <h1>Confirm your email address</h1>
      <p class="lede">
        Your sign-in worked, but your email address is not confirmed yet. Open the
        confirmation link your sign-in provider sent, then sign in again.
      </p>
      {props.held ? (
        <p>
          Your invitation is held for <span>{props.hoursRemaining}</span> more hours.
        </p>
      ) : null}
      {props.requested ? (
        <p class="notice" role="status">
          A new confirmation message was requested. The previous link no longer works.
        </p>
      ) : null}
      {props.unavailable ? (
        <p class="notice" role="status">
          A new message could not be requested right now. Try again later.
        </p>
      ) : null}
      {props.resendAvailable ? (
        <form method="post" action="/auth/pending/resend">
          <input type="hidden" name="csrf" value={props.csrfResend} />
          <button type="submit">Request a new confirmation message</button>
        </form>
      ) : (
        <p>
          You can request another message in <span>{props.cooldownSeconds}</span> seconds.
        </p>
      )}
      <form method="post" action="/auth/signout">
        <input type="hidden" name="csrf" value={props.csrfSignout} />
        <button type="submit">Sign out</button>
      </form>
    </main>
  );
}

mount(
  () =>
    Pending({
      csrfResend: '',
      csrfSignout: '',
      resendAvailable: false,
      cooldownSeconds: 0,
      held: false,
      hoursRemaining: 0,
      requested: false,
      unavailable: false,
    }),
  '#app',
);
