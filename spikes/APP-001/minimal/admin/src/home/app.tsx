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
      <h1>minimal</h1>
      <p class="lede">
        Rust server, TypeScript frontend, no virtual DOM. This paragraph is in
        the HTML the server sends — view source to check.
      </p>
      <Counter count={props.count} />
      <p><a href="/about">About this app</a></p>
    </main>
  );
}

await activateIslands({ Counter });
// SSR uses this call to select Home, then removes it from the browser product.
// An explicit Browser build retains the mount and its independent zero default.
mount(() => Home({ count: 0 }), '#app');
