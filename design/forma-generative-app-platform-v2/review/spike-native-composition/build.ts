import { build } from '@getforma/build';

const isSsr = process.argv.includes('--ssr');

await build({
  app: 'composition-spike',
  entryPoints: [
    { entry: 'src/home/app.tsx', outfile: 'home.js' },
    { entry: 'src/about/app.tsx', outfile: 'about.js' },
  ],
  cssEntries: [
    { input: 'src/styles/app.css', outfile: 'app.css' },
  ],
  routes: {
    '/': { js: ['home'], css: ['app'] },
    '/about': { js: ['about'], css: ['app'] },
  },
  outputDir: 'dist',
  ssr: isSsr,
  // Keys select native page owners before browser outputs are filtered.
  // About keeps its typed native module and shared CSS, with no JS in SSR mode.
  ...(isSsr ? { ssrEntryPoints: {
    home: 'src/home/app.tsx',
    about: 'src/about/app.tsx',
  } } : {}),
  // Unsupported native source is a mandatory coded refusal. Strict mode also
  // rejects advisory diagnostics; FORMA_STRICT=1 enables it without the flag.
  ...(process.argv.includes('--strict') ? { strict: true } : {}),
  ...(process.argv.includes('--watch') ? { watch: true } : {}),
});
