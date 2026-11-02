# App-page drag handle repair

United apps now show the same adaptive Move handle as cards and widgets when selected. Pointer dragging moves the complete app; the existing arrow-key controls remain available. The app content still scrolls inside its window. This is a canvas-island presentation change, with no API, authority, schema, dependency or saved-layout format change.

The defect had three causes: the united frame disabled its standard chrome, its stylesheet clipped the outer canvas item, and sizing also applied inline outer clipping. The fix enables the existing chrome and confines clipping to the inner page. The existing rename event now updates the item and Move labels as well.

[Independent code review](review.md) found and verified repairs for inline clipping and stale rename labels. [Actual local browser observations](browser.json) verify visible/hit-testable chrome, whole-app pointer dragging, arrow movement, saved-position reload, resizing with scroll containment, and an unsent form draft surviving movement/fan-out/reunite. This fixture used the ordinary template picker with a synthetic local member; it is not a hosted authentication or customer-data claim.

Final affected checks: `just check admin` passed TypeScript typecheck, all 103 frontend tests and strict native build `b1d6b7510985`. `just check test http-smoke` passed 375 library tests, eight binary tests, three doctests and native HTTP/security smokes. [All 20 full-check steps passed](checks.json). The affected frontend/native/HTTP steps were rerun on final source after the review repairs. Exact-candidate CI, merge and AWS release results are recorded separately when complete.

The browser check covers the actual clipping/interaction regression rather than a test that merely asserts the presence of a source option. APP-142's older predecessor and phase gates remain unchanged.
