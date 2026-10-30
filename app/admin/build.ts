import { build } from '@getforma/build';

const isSsr = process.argv.includes('--ssr');

await build({
  app: 'forma-app-platform',
  entryPoints: [
    { entry: 'src/home/app.tsx', outfile: 'home.js' },
    { entry: 'src/about/app.tsx', outfile: 'about.js' },
    { entry: 'src/invite/app.tsx', outfile: 'invite.js' },
    { entry: 'src/signin/app.tsx', outfile: 'signin.js' },
    { entry: 'src/pending/app.tsx', outfile: 'pending.js' },
    { entry: 'src/denied/app.tsx', outfile: 'denied.js' },
    { entry: 'src/workspace/app.tsx', outfile: 'workspace.js' },
    { entry: 'src/record/app.tsx', outfile: 'record.js' },
    { entry: 'src/work/app.tsx', outfile: 'work.js' },
    { entry: 'src/canvas/app.tsx', outfile: 'canvas.js' },
  ],
  cssEntries: [
    { input: 'src/styles/app.css', outfile: 'app.css' },
    // ADR-011: the vendored GenUI canvas geometry styles and the pitch canvas.
    { input: 'src/canvas-vendor/canvas.css', outfile: 'canvas-vendor.css' },
    { input: 'src/styles/canvas.css', outfile: 'canvas.css' },
    // ADR-011 scenarios: the confirmation card and the records card.
    { input: 'src/styles/actions.css', outfile: 'actions.css' },
    // APP-102: the receipt attachment and the expense ledger.
    { input: 'src/styles/ledger.css', outfile: 'ledger.css' },
    // APP-103: the utility widgets and their closed set of skins.
    { input: 'src/styles/widgets.css', outfile: 'widgets.css' },
  ],
  // APP-009 (ADR-006): the sign-in page also serves /signin/{handle}; the
  // denial page is rendered by the callback and the pending route. APP-074:
  // the invitation entry also renders POST /invite/validate's refusals.
  routes: {
    '/': { js: ['home'], css: ['app'] },
    '/about': { js: ['about'], css: ['app'] },
    '/invite': { js: ['invite'], css: ['app'] },
    '/signin': { js: ['signin'], css: ['app'] },
    '/auth/pending': { js: ['pending'], css: ['app'] },
    '/auth/callback': { js: ['denied'], css: ['app'] },
    '/workspace': { js: ['workspace'], css: ['app'] },
    // APP-013: the record page serves /work/{app}/{work}/records/{collection}/{record}.
    '/record': { js: ['record'], css: ['app'] },
    // APP-012: the work page (GET and POST /w/{app}/{work}).
    '/w/{app}/{work}': { js: ['work'], css: ['app'] },
    // ADR-011 (APP-092): the pitch canvas, one island; a declared canvas
    // route (365 KiB JavaScript ceiling; Forma's own code stays at N2's).
    '/canvas': { js: ['canvas'], css: ['app', 'canvas-vendor', 'canvas', 'actions', 'ledger', 'widgets'], canvas: true },
  },
  outputDir: 'dist',
  ssr: isSsr,
  // Keys select native page owners before browser outputs are filtered.
  // About and the APP-009 pages keep their typed native modules and shared
  // CSS, with no JS in SSR mode.
  ...(isSsr ? { ssrEntryPoints: {
    home: 'src/home/app.tsx',
    about: 'src/about/app.tsx',
    invite: 'src/invite/app.tsx',
    signin: 'src/signin/app.tsx',
    pending: 'src/pending/app.tsx',
    denied: 'src/denied/app.tsx',
    workspace: 'src/workspace/app.tsx',
    record: 'src/record/app.tsx',
    work: 'src/work/app.tsx',
    canvas: 'src/canvas/app.tsx',
  } } : {}),
  // Unsupported native source is a mandatory coded refusal. Strict mode also
  // rejects advisory diagnostics; FORMA_STRICT=1 enables it without the flag.
  ...(process.argv.includes('--strict') ? { strict: true } : {}),
  ...(process.argv.includes('--watch') ? { watch: true } : {}),
});
