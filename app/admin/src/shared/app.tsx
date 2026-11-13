import { PresentationHeader } from '../presentation/header';
import { h, mount, createShow, activateIslands } from '@getforma/core';
import type { SharedPageProps } from './types';
import { SharedParts } from './parts';
import { startSharedPage } from './document';

function SharedControls(props: { canAccept: boolean; canAddToCanvas: boolean; addedToCanvas: boolean; available: boolean }) {
  return <div class="shared-actions">
    {createShow(() => props.canAccept, () => <button type="button" class="share-primary" data-share-accept="" onClick={() => document.dispatchEvent(new CustomEvent('forma:shared-action', { detail: 'accept' }))}>Accept app invitation</button>)}
    {createShow(() => props.canAddToCanvas, () => <button type="button" class="share-primary" data-share-add="" hidden={props.addedToCanvas} onClick={() => document.dispatchEvent(new CustomEvent('forma:shared-action', { detail: 'add' }))}>Add to my canvas</button>)}
    {createShow(() => props.addedToCanvas, () => <a class="share-secondary" data-share-open="" href="/shared">Open my connected apps</a>)}
    {createShow(() => props.available, () => <button type="button" class="share-secondary" onClick={() => document.dispatchEvent(new CustomEvent('forma:shared-action', { detail: 'refresh' }))}>Refresh</button>)}
    <p role="status" aria-live="polite" data-shared-status=""></p>
  </div>;
}
function SharedPage(props: SharedPageProps) {
  return <main class="page work shared-page composed-app" data-shared-page="" data-presentation={props.presentation.json} data-app-mode={props.presentation.mode} data-app-accent={props.presentation.accent} data-app-density={props.presentation.density} data-app-font={props.presentation.font} data-share={props.shareId}>
    <a class="forma-brand" href="/" aria-label="Forma home"><span class="forma-mark-tile" aria-hidden="true"><span class="forma-mark"></span></span><span>forma<span class="forma-dot">.</span></span></a>
    <header class="shared-page-head"><p class="share-policy">Private · Live · View only</p><h1>{props.title}</h1>
      {createShow(() => props.available, () => <p class="shared-by">Shared by {props.ownerLabel} · Access expires {props.expiresAt}</p>)}
      <p data-shared-message="">{props.message}</p>
      {/* APP-098 DS-84: the views read the library's synthetic sample data; the records are members' own. Same words as SAMPLE_NOTE_TEXT. */}
      {createShow(() => props.sampleData, () => <p class="sample-note" data-sample-data=""><span class="badge">Sample data</span> Charts and tables here read Forma's synthetic sample data. Records listed were filed by the app's members.</p>)}
    </header>
    {createShow(() => props.needsSignIn, () => <form method="post" action={props.signInAction} class="shared-actions"><input type="hidden" name="csrf" value={props.signInCsrf} /><button type="submit" name="intent" value="sign_in" class="share-primary">Sign in</button>{createShow(() => props.canSignUp, () => <button type="submit" name="intent" value="sign_up" class="share-secondary">Create account</button>)}</form>)}
    <SharedControls canAccept={props.canAccept} canAddToCanvas={props.canAddToCanvas} addedToCanvas={props.addedToCanvas} available={props.available} />
    <PresentationHeader presentation={props.presentation} />
    <div data-shared-content="">{createShow(() => props.available, () => <SharedParts parts={props.parts} views={props.views} recordCards={props.recordCards} recordsTruncated={props.recordsTruncated} />)}</div>
    <footer class="shared-footnote">A connected view of the original app. Changes and access remain with its owner.</footer>
    {createShow(() => props.canSignOut, () => <form method="post" action="/auth/shared/signout"><input type="hidden" name="csrf" value={props.csrfSignout} /><button type="submit" class="share-secondary">Sign out of shared apps</button></form>)}
  </main>;
}
await activateIslands({ SharedControls });
startSharedPage();
mount(() => SharedPage({ presentation: { hasDescription: false, description: '', mode: 'default', accent: 'default', density: 'default', font: 'default', actions: [], json: '{}' }, title: '', shareId: '', status: '', ownerLabel: '', expiresAt: '', canAddToCanvas: false,
  addedToCanvas: false, available: false, message: '', needsSignIn: false, canSignUp: false, canAccept: false, signInAction: '', signInCsrf: '',
  canSignOut: false, csrfSignout: '',
  parts: [], views: [], recordCards: [], recordsTruncated: false, sampleData: false }), '#app');
