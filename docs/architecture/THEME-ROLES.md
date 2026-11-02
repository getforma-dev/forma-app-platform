# Theme roles

APP-166. The canvas, the native pages, an app, a card highlight and a widget share one set of color roles. A named theme is a complete assignment of those roles on `[data-theme="name"]`. Nesting is ordinary CSS: an unset role is inherited, and a level that wants its own identity redeclares every role.

This is the swap DaisyUI demonstrates with `data-theme`. DaisyUI and Tailwind are not dependencies. Shape still returns closed names. A model cannot author CSS, class names or hex values.

A custom property inherits its computed color. The `--cv-*` and native aliases are therefore declared again on `[data-theme]`, so a nested theme resolves them against its own roles.

## Default

`site` and `site-dark` are the paper, charcoal and lime identity from [FORMA-BRAND.md](FORMA-BRAND.md). `:root` is `site`. When the operating system is dark and no element has set `data-theme`, `:root` uses `site-dark`. That is the platform default.

The canvas header offers five palettes. Forma (`site`) is the default. Ink is slate and teal, Harbor is blue-gray and navy, Stone is warm stone and umber, and Grove is mist and forest. The choice is stored in this browser and applied to the canvas document, including the create-app dialog. It follows the operating system's light or dark block. Sign-in and the other native pages stay Forma, because they ship no script. Making one of these the platform default is still a separate decision.

## Who sets a theme

| Level | Attribute | Owns | Leaves alone |
| --- | --- | --- | --- |
| Platform | none, until a default is chosen | Shell, board, native pages, through the role aliases | Grants, geometry, vendor hit-testing |
| App | `data-theme` from Shape's `accent` and `mode`, on the unite frame and that work's cards | Surface, accent, chart and the mark | Other apps, the shell, a card's view and receipts |
| Published app | `data-app-mode` and `data-app-accent` | The admitted appearance (APP-157) | Session Shape does not paint over an element that already has `data-app-mode` |
| Card highlight | `data-palette` and `data-highlight` | `--shape-accent` / `--mark-*` on a row or a bar | The card chrome |
| Widget | `data-skin`, `data-accent`, `data-font` | Skin owns the face. An accent theme can recolor a widget that still says `default` | An explicit widget accent, and the skin's geometry |

`comfortable` is a spacing step between compact and spacious. It is not a color.

## Measure

Color themes share one layout scale: `--space-1` through `--space-6` (4px steps), `--radius-1` through `--radius-3` (4, 6 and 8px), `--control` (40px), and `--font-ui` / `--font-meta`. A theme does not change that rhythm. The create-app dialog is mounted on the document, outside the shell, and uses this scale plus the color roles. Older canvas rules still carry their own pixel values. New overlay UI should use the scale.

Dark widget faces keep a luminous glow for the same accent name. Paper and retro use the dark ink of that name, so the light face stays readable when the app theme is dark. Those are two surfaces of one name, not a second palette the model can invent.

## Applying an app theme

Shape's closed lists are unchanged. `themeName` in `talk-view.ts` turns them into a theme:

- density alone inherits the platform
- mode `dark` or `light` with accent `default` selects `site-dark` or `site`
- an accent with mode `default` follows the operating system, and follows it again if the system scheme changes during the session
- an explicit mode wins over the system

The choice stays in the session map. APP-157 remains the published appearance. This note does not store a theme on the server, and it does not change a grant.
