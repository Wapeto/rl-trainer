# Report relay

A tiny Cloudflare Worker that takes reports from RL Trainer's Feedback page and posts them to a
Discord channel. The Discord webhook stays a secret on the Worker, so it is not in the app.
Free Cloudflare and Discord accounts are enough.

## Set up

1. Make a free account at <https://dash.cloudflare.com/sign-up>.
2. In Discord, make a **Forum** channel for reports (each report becomes a post), then Edit
   Channel, Integrations, Webhooks, New Webhook,
   Copy Webhook URL.
3. From this folder:

   ```
   npx wrangler login
   npx wrangler secret put DISCORD_WEBHOOK_URL    # paste the webhook URL
   npx wrangler deploy
   ```

4. Copy the address `deploy` prints and add `/report` to it
   (`https://rl-trainer-report-relay.<you>.workers.dev/report`).
5. In the GitHub repo: Settings, Secrets and variables, Actions, Variables, New repository
   variable `RL_REPORT_RELAY_URL` with that address. Release builds pick it up.

If someone abuses the relay, make a new webhook in Discord and run `secret put` again.
No app update is needed.

## Install counter

`POST /ping` with a JSON body `{"id": "<32 hex characters>", "version": "0.3.0"}` and the
`X-RL-Trainer-Version` header. The app sends it once a day (Settings, "Count this install").
Each id is kept in a KV namespace for 30 days after its last ping; nothing else is stored.

1. `npx wrangler kv namespace create INSTALLS`
2. Paste the `id` it prints into `wrangler.toml` (`[[kv_namespaces]]`).
3. `npx wrangler deploy`

Count the installs seen in the last 30 days:

```
npx wrangler kv key list --binding INSTALLS --remote
```

## Rules

`POST /report`, multipart, with an `X-RL-Trainer-Version` header. Fields: `kind` (bug or idea),
`text` (1 to 2000 characters), `contact` (up to 64), `version`, `windows`, optional `screenshot`
(PNG or JPEG, up to 8 MB) and `log` (up to 300 KB), 9 MB in all. Three reports per minute per IP (pings share the limit).
The same limits are checked in `crates/rl-app/src/report.rs`.

## Test

```
node --test
```
