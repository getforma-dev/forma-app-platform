import { h, mount } from '@getforma/core';

// Native workspace entry. Routing is validated on the server; account
// credentials remain on the provider's sign-in page, never in this form.
type SigninProps = { action: string; hasAction: boolean; failed: boolean; workspace: string };

function Signin(props: SigninProps) {
  return (
    <main class="page signin-page">
      <a class="forma-brand" href="/" aria-label="Forma home"><span class="forma-mark-tile" aria-hidden="true"><span class="forma-mark"></span></span><span>forma<span class="forma-dot">.</span></span><span class="forma-product">canvas</span></a>
      <h1>Sign in to your workspace</h1>
      <p class="lede">Your apps and team, in one place.</p>
      {props.failed ? (
        <p class="notice" role="alert">
          Sign-in could not continue. Check your workspace name or sign-in link and try again.
        </p>
      ) : null}
      <form method="post" action="/signin" class="entry signin-entry">
        <label for="signin-workspace">Workspace</label>
        {h('input', {
          id: 'signin-workspace', name: 'workspace', type: 'text', required: true,
          value: props.workspace, maxlength: 512, autocomplete: 'off',
          autocapitalize: 'none', spellcheck: 'false', inputmode: 'text',
          placeholder: 'Workspace name or sign-in link', 'aria-describedby': 'signin-help',
        })}
        <p class="help" id="signin-help">
          Enter the workspace name or sign-in link your team shared. You’ll enter your account details on the next page.
        </p>
        <button type="submit">Sign in</button>
      </form>
      {props.hasAction ? <p class="signin-switch"><a href="/signin?switch=1">Switch workspace</a></p> : null}
      <p class="signin-invite">New here? <a href="/invite">Enter your invitation code</a>.</p>
      <p class="signin-back"><a href="/">Back to Forma</a></p>
    </main>
  );
}

// The native build uses Signin as its typed root and publishes no JS.
mount(() => Signin({ action: '', hasAction: false, failed: false, workspace: '' }), '#app');
