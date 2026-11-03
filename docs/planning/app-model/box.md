# What a box is

Spike APP-149. Read from APP-108, APP-120, and `app/src/boxes/`. The folder code is unchanged.

## What it is today

A box is a folder widget. The member says folder, box, or storage (`app/src/boxes/intent.rs`). It holds files the member uploaded. It is not an admitted app and it has no cards. Asking about a folder uses the folder ask route, `POST /v1/boxes/{box}/ask` (`app/src/boxes/ask.rs`). That is not `POST /v1/ask`.

## How an app comes out of it

Formation is `/folder <name|number> formation <instruction>` (`app/src/boxes/mod.rs`). It reads the folder and produces an app through the existing composer and admission path. The new app links back to the folder. The folder remains a widget. Model output from formation is data, not markup, SQL, FMIR, or a grant.

## Where it sits

APP-141 creation does not require a folder. A folder is an optional way to start an app from files. A component is not a file. A widget that is not a folder does not hold files.

## Keep or drop

Keep. APP-108 shipped the folder, and APP-120 already placed it as a widget whose formation produces an app. Removing it would drop file holding, folder ask, and formation. Victor has not asked to remove it. This spike does not remove it.
