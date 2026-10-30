# Slack setup for the pitch (APP-100)

You do steps 1–4 once. The secret values stay with you: they go into the clone's gitignored `.env` and into the `SlackApp` secret. They never go in chat, a commit or a log.

1. **Create the app.**
   1. Open <https://api.slack.com/apps>, click **Create New App**, then **From a manifest**.
   2. Pick the workspace for the demo and paste [`slack-app-manifest.json`](slack-app-manifest.json).
   3. Click **Create**.

   The slash command and interactivity URLs point at `https://beta.getforma.dev`. Slack does not call them until someone uses the app, so you can create it before the pitch deploy is live.
2. **Install it.** Under **Install App**, click **Install to Workspace**, then **Allow**.
3. **Copy two values into the clone's `.env`.** The file is `forma-pitch-mvp/.env`, mode 0600 and gitignored.

   ```text
   SLACK_BOT_TOKEN=xoxb-…            # Install App → Bot User OAuth Token
   SLACK_SIGNING_SECRET=…            # Basic Information → App Credentials → Signing Secret
   ```

4. **At the pitch deploy**, put the same two values into the `SlackApp` secret yourself, as one JSON object with exactly these keys: `{"bot_token":"xoxb-…","signing_secret":"…"}`. The deploy runbook gives the exact command (`app/scripts/read_env.py --object …` piped into `put-secret-value`), which reads from the `.env` without echoing. Quotes, `export` and Windows line endings are fine; a name written twice is refused. ECS injects the two keys into the service as `SLACK_BOT_TOKEN` and `SLACK_SIGNING_SECRET`; the secret starts as `{"bot_token":"","signing_secret":""}`, which means "no Slack app" (every Slack route answers 404). Force a new deployment after the write.
5. **Link the demo people.**
   1. In Slack, each demo person runs `/forma link` and opens the link while signed in to `beta.getforma.dev` as their Forma member.
   2. They confirm.
   3. Check the result: `/forma what's waiting for my approval` should answer for that person.
   4. A sign-out on the canvas ends the Slack link (it advances the member's epoch). Re-link after one: run `/forma link` again. `/forma unlink` disconnects on purpose.
6. **Rehearse.** Follow the Slack act in [`SCENARIOS.md`](SCENARIOS.md).

## Local testing before the deploy

Slack can only reach a public HTTPS URL. Until the pitch deploy is live, the server is exercised with signed synthetic payloads and a loopback Slack API stub (`storage::slack_campaign`, base URL from `GENUI_SLACK_API_BASE`, a test-only variable). For a live local test, you can run a tunnel (for example `cloudflared tunnel --url http://localhost:3000`) and temporarily point the two URLs in the app's settings at it. Point them back to `beta.getforma.dev` afterwards.
