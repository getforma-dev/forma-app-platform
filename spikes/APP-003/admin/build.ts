import { build } from '@getforma/build';
await build({
  app: 'genui-canvas-qualification',
  entryPoints: [{ entry: 'src/home/app.tsx', outfile: 'home.js' }],
  cssEntries: [{ input: 'src/styles/app.css', outfile: 'app.css' }],
  routes: { '/': { js: ['home'], css: ['app'], canvas: true } },
  outputDir: 'dist', ssr: true,
  ssrEntryPoints: { home: 'src/home/app.tsx' }, strict: true,
});
