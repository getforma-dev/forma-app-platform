# How Forma connects: people, agents and AI

The onboarding explainer. It covers who talks to Forma, how they connect, and whose AI does the thinking. The definitions are in [GUIDE.md](GUIDE.md).

## The three connections

| | Who is calling | Calls | Proves who they are with | Used when |
| --- | --- | --- | --- | --- |
| **The Forma app** (web canvas and Slack) | A person | Forma | Their sign-in (fAuth). In Slack, a one-time `/forma link` to their Forma account | Someone clicks, types, uploads, approves or shares |
| **Your agents** (MCP) | An AI agent: Claude Code, an OpenAI agent, Cursor, your own bot | Forma's MCP server at `/v1/mcp` | An **agent token** the person creates under "Connect an agent" | Someone asks their AI assistant to do work in Forma |
| **Forma's AI** | Forma itself | A language model (Claude, through Anthropic's API) | Forma's own key, held in the cloud's secret store and never shown to anyone | Forma needs to understand a sentence, pick or reshape an app, build from a folder, or write a brief |

## What this means for a team

**People** use Forma directly. Everything they see and change goes through their own permissions.

**Your agents act as you, and never as more than you.**
- An agent connected with your token can list your apps, run views, draft records, file them after you confirm, decide approvals you are allowed to decide, share, and build from your folders.
- It cannot see another team's data. It cannot skip an approval step or approve its own request.
- Every action it takes is recorded and shown with a chip on the canvas, for example "via Claude" or "via OpenAI".
- You can narrow a token when you create it: only some spaces (for example Security), only some actions (read, draft, commit, decide, share, build apps), and an expiry of up to 30 days. A scope never adds a right you do not have.
- Every call an agent makes is logged: what it touched, whether it was allowed, and why it was refused. See it under "Connect an agent" → **Activity**.
- To revoke a token, use "Connect an agent". Signing out also ends your tokens.

**Forma's AI** never touches permissions or data directly.
- The model proposes typed definitions, drafts and summaries. Forma checks each one and either admits it or refuses it whole.
- The model never writes code, SQL, page markup or permissions.
- Numbers always come from the server's own calculations, with receipts. A summary can describe a number but cannot invent one.
- The model sees schemas and small samples, never who is asking.

## Bring your own AI, or use ours

| Option | What it means | Status |
| --- | --- | --- |
| **Use Forma's AI** | AI is included. Forma calls the model with its own key. | Built: the default today |
| **Bring your own agent** | Your agents (Claude, ChatGPT, Cursor, your own) work in Forma through MCP, with your permissions | Built: Claude Code and any MCP client with a bearer token. claude.ai and ChatGPT connector screens need OAuth, which is planned (APP-098 DS-48) |
| **Bring your own model key (BYOK)** | Forma's AI features run on your organization's own model account: Anthropic, AWS Bedrock or Azure OpenAI, in your region and under your contract | Planned for enterprise (APP-110 section C, APP-114) |

## How to connect

1. **People:** open your organization's sign-in link and sign in. Nothing else is needed.
2. **Slack:** an admin installs the Forma app once. Then each person runs `/forma link` and confirms in the browser while signed in to Forma.
3. **Claude Code:**
   1. On the canvas, open **Connect an agent** and create a token. It is shown once; copy it.
   2. Put the `.mcp.json` snippet from that panel in your project.
   3. In your shell, run `read -rs FORMA_AGENT_TOKEN && export FORMA_AGENT_TOKEN` and paste the token. It is not echoed and not stored in your shell history.

   The token never goes on a command line.
4. **An OpenAI agent:** add the tools entry from the same panel to your agent's code, reading the token from your own secret storage.
5. **Your own agent:** any client that speaks MCP over HTTP with a bearer token works the same way.

## Which one is used when

- Diego types "I spent $86 at Nobu, Sarah approves" on the canvas. That is **the Forma app**; Forma's AI reads the sentence into a typed draft, and Diego confirms.
- Sarah asks Claude, "what's waiting for me? approve the Nobu dinner". That is **her agent through MCP**, acting with her rights. Diego's canvas updates live through **the Forma app**.
- Someone runs `/folder formation build me a metrics app`. **Forma's AI** proposes the app, and Forma checks it, computes its numbers and records where every number came from.
