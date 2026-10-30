import { h, Fragment, mount, activateIslands, createSignal } from '@getforma/core';

type HomeProps = { count: number };

function Counter(props: HomeProps) {
  const [count, setCount] = createSignal(props.count);
  return (
    <section aria-label="Counter">
      <p class="count">
        Count: <span>{() => count()}</span>
      </p>
      <p class="actions">
        <button onClick={() => setCount(count() + 1)}>Increment</button>
        <button onClick={() => setCount(0)}>Reset</button>
      </p>
    </section>
  );
}

function Home(props: HomeProps) {
  return (
    <main class="page">
      <h1>Forma app platform</h1>
      <p class="lede">
        Foundation workspace. The beta is by invitation only: enter the code you
        were sent to create your account, or sign in with your organization's link.
      </p>
      <nav aria-label="Beta access" class="entry-links">
        <a href="/invite">Enter an invitation code</a>
        <a href="/signin">Sign in</a>
      </nav>
      <Counter count={props.count} />
      <p><a href="/about">About this app</a></p>
    </main>
  );
}

await activateIslands({ Counter });
// SSR uses this call to select Home, then removes it from the browser product.
// An explicit Browser build retains the mount and its independent zero default.
mount(() => Home({ count: 0 }), '#app');
