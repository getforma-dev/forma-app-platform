# Theme roles

APP-166. The canvas, the native pages, an app, a card highlight and a widget share one set of color roles. A named theme is a complete assignment of those roles on `[data-theme="name"]`. Nesting is ordinary CSS: an unset role is inherited, and a level that wants its own identity redeclares every role.

This is the swap DaisyUI demonstrates with `data-theme`. DaisyUI and Tailwind are not dependencies. Shape still returns closed names. A model cannot author CSS, class names or hex values.

A custom property inherits its computed color. The `--cv-*` and native aliases are therefore declared again on `[data-theme]`, so a nested theme resolves them against its own roles.

## Default

`site` and `site-dark` are the paper, charcoal and lime identity from [FORMA-BRAND.md](FORMA-BRAND.md). `:root` is `site`. When the operating system is dark and no element has set `data-theme`, `:root` uses `site-dark`. That is the platform default.

The bottom bar, beside “Layout saved locally,” offers Day and Dark plus three palettes: Ink (slate and teal), Harbor (blue-gray and navy), and Stone (warm stone and umber). None pressed is Forma, the default. Pressing the active palette returns to Forma. Day and Dark are explicit; until one is pressed, the bar follows the operating system. The choice stays in this browser and covers the canvas, including the create-app dialog. Grove remains available to Shape. Sign-in and the other native pages stay Forma, because they ship no script.

## Who sets a theme

| Level | Attribute | Owns | Leaves alone |
| --- | --- | --- | --- |
| Platform | none, until a default is chosen | Shell, board, native pages, through the role aliases | Grants, geometry, vendor hit-testing |
| App | `data-theme` from Shape's `accent` and `mode`, on the unite frame and that work's cards | Surface, accent, chart and the mark | Other apps, the shell, a card's view and receipts |
| Published app | `data-app-presentation="authored"`, plus `data-app-mode` and `data-app-accent` | The admitted version-1 appearance (APP-157) | Browser Shape does not paint over an authored design. Default renderer tokens (`data-app-presentation="default"`) can receive a local theme. |
| Card highlight | `data-palette` and `data-highlight` | `--shape-accent` / `--mark-*` on a row or a bar | The card chrome |
| Widget | `data-skin`, `data-accent`, `data-font` | Skin owns the face. An accent theme can recolor a widget that still says `default` | An explicit widget accent, and the skin's geometry |

`comfortable` is a spacing step between compact and spacious. It is not a color.

## Measure

Color themes share one layout scale: `--space-1` through `--space-6` (4px steps), `--radius-1` through `--radius-3` (4, 6 and 8px), `--control` (40px), and `--font-ui` / `--font-meta`. A theme does not change that rhythm. The create-app dialog is mounted on the document, outside the shell, and uses this scale plus the color roles. Older canvas rules still carry their own pixel values. New overlay UI should use the scale.

Dark widget faces keep a luminous glow for the same accent name. Paper and retro use the dark ink of that name, so the light face stays readable when the app theme is dark. Those are two surfaces of one name, not a second palette the model can invent.

## Applying an app theme

`themeName` in `talk-view.ts` turns a judged theme into a theme name:

- density alone inherits the platform
- mode `dark` or `light` with accent `default` selects `site-dark` or `site`
- an accent with mode `default` follows the operating system, and follows it again if the system scheme changes during the session
- an explicit mode wins over the system

Shape can select those same names. `ink`, `harbor`, `stone`, and `grove` are whole palettes. The older color names still recolor an app. Card highlights stay on the color names, so a row mark does not become a second chrome.

An applied app look is stored in this browser under `forma.canvas.app-theme.v1` for the signed-in member. Refresh, sign-out, and sign-in on this browser bring it back. Sign-out does not clear it. A different browser does not have it. The published app design, saved from Edit app, is the copy that follows the member to another browser. A widget's skin, accent, and font are already stored the same way, in `forma.widgets.v1`. The canvas palette is `forma.canvas.palette.v1`. Demo mode neither restores nor writes these browser choices. Revert is restored with each applied app theme and removes that member’s stored override. If an admitted design is adopted while a local preview is open, the design clears the local attributes and controls; a late Shape response cannot replace it.
