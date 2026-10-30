import { h, mount } from '@getforma/core';

// The protected workspace entry: rendered only for a live workspace session.
// It shows no organization data in Phase 1's APP-009 slice.
type WorkspaceProps = { csrfSignout: string; csrfSignoutAll: string };

function Workspace(props: WorkspaceProps) {
  return (
    <main class="page">
      <h1>Workspace</h1>
      <p class="lede">You are signed in.</p>
      <form method="post" action="/auth/signout">
        <input type="hidden" name="csrf" value={props.csrfSignout} />
        <button type="submit">Sign out</button>
      </form>
      <form method="post" action="/auth/signout-all">
        <input type="hidden" name="csrf" value={props.csrfSignoutAll} />
        <button type="submit">Sign out everywhere</button>
      </form>
    </main>
  );
}

mount(() => Workspace({ csrfSignout: '', csrfSignoutAll: '' }), '#app');
