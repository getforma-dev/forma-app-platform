import { build } from '@getforma/build';

const isSsr = process.argv.includes('--ssr');

await build({
  app: 'forma-app-platform',
  entryPoints: [
    { entry: 'src/home/app.tsx', outfile: 'home.js' },
    { entry: 'src/about/app.tsx', outfile: 'about.js' },
    { entry: 'src/signin/app.tsx', outfile: 'signin.js' },
    { entry: 'src/pending/app.tsx', outfile: 'pending.js' },
    { entry: 'src/denied/app.tsx', outfile: 'denied.js' },
    { entry: 'src/workspace/app.tsx', outfile: 'workspace.js' },
    { entry: 'src/work/app.tsx', outfile: 'work.js' },
  ],
  cssEntries: [
    { input: 'src/styles/app.css', outfile: 'app.css' },
  ],
  // APP-009 (ADR-006): the sign-in page also serves /signin/{handle}; the
  // denial page is rendered by the callback and the pending route.
  routes: {
    '/': { js: ['home'], css: ['app'] },
    '/about': { js: ['about'], css: ['app'] },
    '/signin': { js: ['signin'], css: ['app'] },
    '/auth/pending': { js: ['pending'], css: ['app'] },
    '/auth/callback': { js: ['denied'], css: ['app'] },
    '/workspace': { js: ['workspace'], css: ['app'] },
    // APP-012: the work page (GET and POST /w/{app}/{work}).
    '/w/{app}/{work}': { js: ['work'], css: ['app'] },
  },
  outputDir: 'dist',
  ssr: isSsr,
  // Keys select native page owners before browser outputs are filtered.
  // About and the APP-009 pages keep their typed native modules and shared
  // CSS, with no JS in SSR mode.
  ...(isSsr ? { ssrEntryPoints: {
    home: 'src/home/app.tsx',
    about: 'src/about/app.tsx',
    signin: 'src/signin/app.tsx',
    pending: 'src/pending/app.tsx',
    denied: 'src/denied/app.tsx',
    workspace: 'src/workspace/app.tsx',
    work: 'src/work/app.tsx',
  } } : {}),
  // Unsupported native source is a mandatory coded refusal. Strict mode also
  // rejects advisory diagnostics; FORMA_STRICT=1 enables it without the flag.
  ...(process.argv.includes('--strict') ? { strict: true } : {}),
  ...(process.argv.includes('--watch') ? { watch: true } : {}),
});
