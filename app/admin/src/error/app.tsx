import { h, mount } from '@getforma/core';
import { SiteFooter } from '../site/footer';

// APP-217: the themed page a browser navigation gets in place of a shared
// problem+json refusal (http::errors). No page reads a request value: the
// handler passes the status, the closed copy for that refusal and viewer
// variant, and which buttons to show. The path, query, ids and names never
// reach these props, so an unknown path and a protected one stay one answer
// (ADR-006). No script, no form, no cookie. The mascot's surge is CSS only:
// a visually hidden checkbox, and a label around the mascot. The checkbox
// sits after the buttons, so the Tab order is the brand link, the buttons,
// then the mascot toggle; the styles reach it with `.lab:has(...)`.
// APP-223: the shared site footer (`../site/footer`) closes the page; it is
// static, so every refusal and viewer keeps one footer.
type ErrorProps = {
  status: number;
  title: string;
  text: string;
  signIn: boolean;
  workspace: boolean;
};

function ErrorPage(props: ErrorProps) {
  return (
    <div class="lab">
      <main class="lab-card" aria-labelledby="lab-title">
        <div class="lab-stripes" aria-hidden="true"></div>
        <a class="forma-brand" href="/" aria-label="Forma home"><span class="forma-mark-tile" aria-hidden="true"><span class="forma-mark"></span></span><span>forma<span class="forma-dot">.</span></span><span class="forma-product">canvas</span></a>
        <div class="lab-body">
          <label class="lab-mascot" for="lab-surge">
            <img class="lab-mascot-image" src="/_assets/forma-isotope.svg" width="800" height="1090" alt="Forma's isotope mascot, a yellow-suited explorer holding a glowing green capsule" />
            <span class="lab-capsule-glow" aria-hidden="true"></span>
          </label>
          <div class="lab-copy">
            <p class="lab-eyebrow"><span class="lab-hazard" aria-hidden="true">☢</span> CONTAINMENT BREACH · ERROR <span>{props.status}</span></p>
            <h1 id="lab-title">{props.title}</h1>
            <p class="lab-text">{props.text}</p>
            {props.signIn ? (
              <p class="lab-actions">
                <a class="lab-button lab-primary" href="/signin">Sign in</a>
                <a class="lab-button" href="/invite">I have an invitation code</a>
              </p>
            ) : null}
            {props.workspace ? (
              <p class="lab-actions">
                <a class="lab-button lab-primary" href="/">Back to your workspace</a>
                <a class="lab-button" href="/signin">Sign in again</a>
                <span class="lab-note">if your session ended</span>
              </p>
            ) : null}
            <input class="lab-surge-toggle" type="checkbox" id="lab-surge" aria-label="Start the reactor surge, just for fun" />
          </div>
        </div>
        <div class="lab-stripes lab-stripes-end" aria-hidden="true"></div>
      </main>
      <div class="lab-surge" aria-hidden="true">
        <div class="lab-surge-ring"></div>
        <p class="lab-surge-readout"><span class="lab-surge-symbol">☢</span><span>CONTAINMENT BREACH</span><span>REACTOR SURGE · CONTAINED</span></p>
      </div>
      <SiteFooter />
    </div>
  );
}

// The native build uses ErrorPage as its typed root and publishes no JS.
mount(() => ErrorPage({ status: 404, title: '', text: '', signIn: false, workspace: false }), '#app');
