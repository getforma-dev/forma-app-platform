# Forma family: Canvas

APP-138 implements Victor's October 31 direction to give products built on Forma Stack a common visual identity with getforma.dev. This is the current presentation direction; historical design snapshots remain unchanged.

The family uses warm paper (`#f4f4ed`), charcoal (`#1a1d18`), electric lime (`#c5f953`), Space Grotesk and the three-bar geometric F. Small monospaced labels, fine rules and restrained corners connect the productive interface with the technical character of the site. Dark mode uses the site's green-charcoal surfaces. Lime marks primary actions and selection; darker olive supplies accessible text on light surfaces. Status words and receipts retain their own semantic meaning.

The app is presented as **forma. / canvas**. The common F sits in a small lime tile: the tile and product descriptor distinguish this workspace from the unboxed site/Stack identity, without inventing an unrelated logo. This is a presentation label, not a change to API, app or package identifiers. Other suite products can reuse the same wordmark, typography and tokens with their own descriptor.

The welcome screen presents three real paths: describe an app, bring files, or open the library. The canvas remains a working surface with pan/zoom, persistent geometry, explicit confirmation, receipts, Ask and Shape. Controls, dialogs and native forms share the same styling; no decorative control implies a capability that does not exist.

The unmodified Space Grotesk Latin variable font is copied from the local site's `@fontsource-variable/space-grotesk` 5.3.0 asset and licensed under SIL OFL 1.1 (notice beside the font). Forma's existing `fontDir` pipeline serves it from the same origin under the unchanged CSP. No remote font service, runtime dependency or new permission is added.
