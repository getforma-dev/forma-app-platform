import { h, mount, createShow, createList, activateIslands } from '@getforma/core';
import type { SharedCanvasProps } from './types';
import { SharedParts } from './parts';
import { startSharedCanvas } from './canvas-controller';

function ConnectedTools() {
  return <div class="shared-canvas-tools" role="group" aria-label="Canvas controls">
    <button type="button" onClick={() => document.dispatchEvent(new CustomEvent('forma:connected-fit'))}>Fit apps</button>
    <button type="button" onClick={() => document.dispatchEvent(new CustomEvent('forma:connected-refresh'))}>Refresh</button>
    <span role="status" data-connected-status="">Connected apps</span>
  </div>;
}
function SharedCanvas(props: SharedCanvasProps) {
  return <main class="shared-canvas-page" data-shared-canvas="">
    <header class="shared-canvas-head"><a class="forma-brand" href="/" aria-label="Forma home"><span class="forma-mark-tile" aria-hidden="true"><span class="forma-mark"></span></span><span>forma<span class="forma-dot">.</span></span></a>
      <div><p class="share-policy">Your connected apps</p><h1>My canvas</h1><p>{props.accountLabel}</p></div>
      {createShow(() => props.hasWorkspace, () => <a class="share-secondary" href="/workspace">Workspace</a>)}
      {createShow(() => props.canSignOut, () => <form method="post" action="/auth/shared/signout"><input type="hidden" name="csrf" value={props.csrfSignout} /><button type="submit" class="share-secondary">Sign out</button></form>)}
    </header>
    <ConnectedTools />
    <p class="shared-canvas-empty" data-connected-empty="" hidden={props.hasApps}>Apps you add from a private share appear here, connected to their owner’s live data.</p>
    <div class="shared-canvas-surface" data-connected-surface="">
      <div data-connected-initial="">
        {createList(() => props.apps, app => app.id, app => <article class="connected-app" data-connected-app={app.id}>
          <header class="connected-app-head" data-canvas-drag-handle=""><div><p class="share-policy">Connected · View only</p><h2>{app.title}</h2><p class="connected-owner">{app.ownerLabel}</p></div>
            <a href={app.href} class="share-secondary">Open app</a><button type="button" data-connected-remove={app.id}>Remove from canvas</button></header>
          <SharedParts parts={app.parts} views={app.views} recordCards={app.recordCards} recordsTruncated={app.recordsTruncated} />
        </article>)}
      </div>
    </div>
  </main>;
}
await activateIslands({ ConnectedTools });
startSharedCanvas();
mount(() => SharedCanvas({ accountLabel: '', csrfSignout: '', canSignOut: false, hasWorkspace: false, hasApps: false, apps: [] }), '#app');
