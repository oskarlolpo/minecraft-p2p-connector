# Cloudflare TURN Deploy

## Что уже готово

В репозитории есть отдельный worker:

- [apps/cloudflare-turn-worker](G:/minecraftjava/p2p/apps/cloudflare-turn-worker)

Он выдаёт short-lived ICE/TURN credentials для Cloudflare Realtime TURN и нужен для `Cloudflare TURN/WebRTC` fallback в desktop приложении.

## Что нужно от Cloudflare

1. `TURN_KEY_ID`
2. `TURN_KEY_API_TOKEN`
3. авторизация `wrangler login` или GitHub secrets для CI

## Локальный ручной деплой

```powershell
cd G:\minecraftjava\p2p\apps\cloudflare-turn-worker
npm install
npx wrangler login
npx wrangler secret put TURN_KEY_ID
npx wrangler secret put TURN_KEY_API_TOKEN
npx wrangler deploy
```

После этого worker выдаст endpoint вида:

`https://<subdomain>.workers.dev/ice-servers`

## Что прописать в desktop app

Создайте `.env` рядом с [package.json](G:/minecraftjava/p2p/package.json) по шаблону [.env.example](G:/minecraftjava/p2p/.env.example) и укажите:

```env
MC_CF_TURN_CREDENTIAL_ENDPOINT=https://<subdomain>.workers.dev/ice-servers
```

## GitHub Actions

Есть workflow:

- [cloudflare-turn-worker.yml](G:/minecraftjava/p2p/.github/workflows/cloudflare-turn-worker.yml)

Для него нужны secrets:

- `CLOUDFLARE_API_TOKEN`
- `CLOUDFLARE_ACCOUNT_ID`
- `TURN_KEY_ID`
- `TURN_KEY_API_TOKEN`

Тогда worker будет деплоиться через GitHub Actions.
