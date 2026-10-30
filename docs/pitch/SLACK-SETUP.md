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

4. **At the pitch deploy**, put the same two values into the `SlackApp` secret yourself. The deploy runbook gives the exact command, which reads from the `.env` without echoing.
5. **Link the demo people.**
   1. In Slack, each demo person runs `/forma link` and opens the link while signed in to `beta.getforma.dev` as their Forma member.
   2. They confirm.
   3. Check the result: `/forma what's waiting for me` should answer for that person.
6. **Rehearse.** Follow the Slack act in [`SCENARIOS.md`](SCENARIOS.md).

## Local testing before the deploy

Slack can only reach a public HTTPS URL. Until the pitch deploy is live, the server is exercised with signed synthetic payloads and a loopback Slack API stub (`storage::slack_campaign`). For a live local test, you can run a tunnel (for example `cloudflared tunnel --url http://localhost:3000`) and temporarily point the two URLs in the app's settings at it. Point them back to `beta.getforma.dev` afterwards.
