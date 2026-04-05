# Cloudflare TURN Worker

Этот worker выдаёт short-lived ICE/TURN credentials для Cloudflare Realtime TURN.

## Что нужно задать

- `TURN_KEY_ID`
- `TURN_KEY_API_TOKEN`
- `TURN_CREDENTIAL_TTL` (опционально, по умолчанию `3600`)

## Локально

```bash
npm install
npx wrangler secret put TURN_KEY_ID
npx wrangler secret put TURN_KEY_API_TOKEN
npm run dev
```

## Деплой

```bash
npm install
npx wrangler login
npx wrangler secret put TURN_KEY_ID
npx wrangler secret put TURN_KEY_API_TOKEN
npm run deploy
```

После деплоя endpoint для desktop app:

`https://<your-worker-subdomain>.workers.dev/ice-servers`

Именно его нужно передать в переменную окружения:

`MC_CF_TURN_CREDENTIAL_ENDPOINT`
