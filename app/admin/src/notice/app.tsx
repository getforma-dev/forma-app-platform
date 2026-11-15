import { h, mount, createList } from '@getforma/core';
import { SiteFooter } from '../site/footer';

// APP-223: the public "Beta terms & privacy" page, GET /notice. It shows the
// beta notice every member accepts on the invitation page, version and text
// from the one source in `auth::notice` (the same rows `InviteProps::notice`
// builds), so the two pages can never disagree. No auth, no cookie, no form,
// no script.
type NoticeParagraph = { id: string; lead: string; hasLead: boolean; text: string };
type NoticeProps = {
  noticeTitle: string;
  noticeVersion: string;
  notice: NoticeParagraph[];
};

function Notice(props: NoticeProps) {
  return (
    <div class="site">
      <main class="page notice-page">
        <a class="forma-brand" href="/" aria-label="Forma home"><span class="forma-mark-tile" aria-hidden="true"><span class="forma-mark"></span></span><span>forma<span class="forma-dot">.</span></span><span class="forma-product">canvas</span></a>
        <h1>Beta terms &amp; privacy</h1>
        <p class="lede">
          This is the notice every member accepts when joining. Questions: <a href="mailto:feedback@getforma.dev">feedback@getforma.dev</a>.
        </p>
        <section class="beta-notice" aria-labelledby="notice-title" data-notice-version={props.noticeVersion}>
          <p class="page-eyebrow">Beta notice, version <span>{props.noticeVersion}</span></p>
          <h2 id="notice-title">{props.noticeTitle}</h2>
          {createList(() => props.notice, paragraph => paragraph.id, paragraph => (
            <p>
              {paragraph.hasLead ? <strong>{paragraph.lead}</strong> : null}
              {paragraph.hasLead ? ' ' : ''}
              {paragraph.text}
            </p>
          ))}
        </section>
      </main>
      <SiteFooter />
    </div>
  );
}

// The native build uses Notice as its typed root and publishes no JS.
mount(() => Notice({ noticeTitle: '', noticeVersion: '', notice: [] }), '#app');
