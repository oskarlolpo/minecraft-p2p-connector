# Анализ 13 проектов из `G:\minecraftjava\newrepo`

## Цель анализа

Понять, какие из найденных проектов реально помогут построить сильный `Minecraft P2P Connector` для Windows:

- с низким пингом
- без обязательного VPN-адаптера в основном сценарии
- с максимальной устойчивостью в России и СНГ
- с понятным UI
- с архитектурой, пригодной для роста, а не для очередного хрупкого релиза

Ниже по каждому проекту:

- что это такое
- плюсы
- минусы
- подходит нам или нет
- что можно забрать
- как сделать лучше у себя

## Краткий рейтинг полезности

### Tier A: брать как сильный reference

- `e4mc-minecraft-architectury-rererewrite`
- `MinecraftConnectTool-master`
- `ConnectX-main`
- `mainline-main`

### Tier B: брать частично, как технический reference

- `Basic-P2P-TCP-Relayer-main`
- `Hive.Framework-main`
- `go-libp2p-kad-dht-master`
- `p2pmcclient-main`
- `EOS-Getting-Started-main`
- `EOS-SDK-49960398-Release-v1.19.0.3`

### Tier C: почти не подходит для нашего продукта

- `P2P-Minecraft-server-develop`
- `Installer-main`
- `matchbox-main`

---

## 1. Basic-P2P-TCP-Relayer-main

### Что это

Небольшой `.NET`-проект, который прямо заявляет простую цель:

> сделан для Minecraft-сервера, где нельзя открыть порт

По структуре это:

- `P2P_Relayer.CLI`
- `P2P_Relayer.Common`
- `P2P_Relayer.Gateway`

То есть минималистичный relay/proxy стек.

### Плюсы

- очень близок к нашей задаче по смыслу
- не перегружен лишней экосистемой
- понятная идея: host/client/gateway
- можно быстро вытащить логику жизненного цикла relay

### Минусы

- это не готовый “идеальный продукт”
- `.NET`, а не Rust
- нет признаков продвинутой NAT-strategy
- скорее relay-first utility, чем современная transport architecture

### Подходит нам?

Да, но только как reference для минимального relay pipeline и структуры ролей.

### Что подчерпнём

- разделение `client / host / gateway`
- простую терминологию и роли транспортного ядра
- минимальный путь проксирования Minecraft TCP

### Как сделать лучше у себя

- вынести эту роль в `bp-proxy` и `net-helper`
- не ограничиваться TCP relay
- держать direct + fallback, а не только один relay путь

---

## 2. ConnectX-main

### Что это

Кроссплатформенная Minecraft P2P-библиотека на C#.
Сама пишет, что использует:

- high-performance sockets
- Zerotier SDK для P2P
- собственный relay
- Hive.Framework как базу

### Плюсы

- очень близкая доменная область
- уже есть продуманная серверная, relay и client логика
- есть low-latency relay направление
- есть dual-stack socket support
- выглядит как серьёзный инженерный продукт, а не любительский dump

### Минусы

- сильная зависимость от Zerotier-подхода
- для нас это риск смещения в сторону overlay/VPN-like поведения
- C#/.NET экосистема, не Rust
- часть решений завязана на их стек и framework

### Подходит нам?

Да, как очень сильный reference по архитектуре сервиса.

### Что подчерпнём

- разделение на:
  - client
  - server
  - relay
  - shared
- логирование room ops в БД
- собственный relay control plane
- аккуратное разделение transport и coordination

### Как сделать лучше у себя

- повторить modular service layout, но на Rust
- не тащить ZeroTier как основу
- использовать их как образец инженерной дисциплины, а не как буквальную копию

---

## 3. e4mc-minecraft-architectury-rererewrite

### Что это

Самый ценный Minecraft-specific reference в наборе.

Проект делает ровно то, что нам интересно по UX:

> Open a LAN server to anyone, anywhere, anytime.

Это Minecraft-mod решение, не отдельный desktop-коннектор. По README видно связанные компоненты:

- `e4mc-quiclime` — relay-side code
- `iroh-java`
- форк `netty-incubator-codec-quic`
- в коде есть `dialtone`

### Плюсы

- очень близко к целевой боли пользователя
- уже существует рабочая модель “открыть LAN наружу”
- это не абстрактная сеть, а именно Minecraft LAN integration
- важный референс по тому, как прозрачно встроиться в Minecraft flow
- использует QUIC/relay-направление не игрушечно

### Минусы

- это mod ecosystem, а не внешний desktop app
- Java/Architectury/Fabric/Forge/NeoForge стек тяжёлый для прямого переноса
- часть логики завязана на mod-level доступ к клиенту/серверу

### Подходит нам?

Да. Это один из самых полезных проектов во всём списке.

### Что подчерпнём

- UX-модель “обычный Open to LAN, а дальше магия”
- подход к Minecraft-specific интеграции
- transport abstraction вокруг Dialtone / relay path
- паттерны диагностики и handshake около Minecraft session

### Как сделать лучше у себя

- дать такой же простой UX, но без обязательного мода
- оставить внешний desktop app и localhost proxy
- взять их conceptual model, а не Java stack

---

## 4. EOS-Getting-Started-main

### Что это

Официальные примеры Epic Online Services:

- authentication
- presence
- friends
- lobbies
- P2P
- voice

### Плюсы

- официальный reference
- хорошо показывает, как строится lobby/presence/session model
- полезен для product thinking вокруг matchmaking

### Минусы

- EOS не наш целевой транспортный стек
- экосистема Epic слишком тяжёлая и чужая для нашего desktop-коннектора
- не Minecraft-specific

### Подходит нам?

Частично. Брать не как network core, а как reference по lobby/session/presence semantics.

### Что подчерпнём

- структуру presence/lobby state
- отделение authentication / lobby / session / P2P
- нормальный жизненный цикл сетевой сессии

### Как сделать лучше у себя

- взять паттерны состояния, не тащить EOS
- сохранить простой anonymous-first сценарий, а не превращать всё в platform-login

---

## 5. EOS-SDK-49960398-Release-v1.19.0.3

### Что это

Просто пакет EOS SDK:

- `Bin`
- `Include`
- `Lib`
- `Tools`

Это не проект, а vendor SDK bundle.

### Плюсы

- полезно как сырьё, если когда-нибудь захотим экспериментировать с EOS transport/presence

### Минусы

- сам по себе не даёт архитектуры продукта
- избыточен для нашей текущей цели
- может увести в enterprise-heavy сторону

### Подходит нам?

Скорее нет как основа продукта. Только как справочный SDK.

### Что подчерпнём

- практически ничего на архитектурном уровне

### Как сделать лучше у себя

- не строить новый коннектор на EOS

---

## 6. go-libp2p-kad-dht-master

### Что это

Официальная реализация Kademlia DHT в Go для libp2p.

### Плюсы

- качественный reference по peer discovery
- production-grade DHT implementation
- полезен, если захотим уменьшить зависимость от централизованного lobby/signaling

### Минусы

- DHT не решает сам по себе hostile NAT
- discovery != transport
- для нашей боли “Казань за CGNAT” это не серебряная пуля

### Подходит нам?

Частично.

### Что подчерпнём

- идеи peer discovery
- distributed room indexing
- отказ от одного централизованного списка комнат в будущем

### Как сделать лучше у себя

- если использовать DHT, то только как discovery layer
- не превращать DHT в главный ответ на NAT traversal

---

## 7. Hive.Framework-main

### Что это

Китайский open-source game server / networking framework на `.NET`.
Судя по структуре, там есть:

- Networking abstractions
- KCP
- QUIC
- TCP
- UDP
- codecs
- ECS
- data sync

### Плюсы

- сильный инженерный фундамент
- много транспортов под одной abstraction model
- полезен как reference по framework-level modularity

### Минусы

- очень широкий фреймворк, не focused продукт
- не Minecraft-specific
- риск утащить в “строим свой движок вместо сервиса”

### Подходит нам?

Да, но только как reference по modular networking architecture.

### Что подчерпнём

- нормальное разделение:
  - transport abstractions
  - codecs
  - shared utilities
- идею transport-pluggable architecture

### Как сделать лучше у себя

- взять только modularity
- не тащить ECS и огромный framework scope
- оставить продукт узким: Minecraft connector, а не game backend platform

---

## 8. Installer-main

### Что это

Репозиторий `LocalMiner` installer/page materials.
По README это продукт для новых игроков Minecraft, чтобы играть без port forwarding и VPN stuff.

Но по факту это скорее маркетингово-дистрибуционный репозиторий, а не основной код продукта.

### Плюсы

- полезен как product positioning reference
- хорошо формулирует pain point простыми словами

### Минусы

- не даёт полноценного инженерного ядра
- больше похож на installer/landing/support repo
- в техническом смысле пользы мало

### Подходит нам?

Скорее нет как технический reference.

### Что подчерпнём

- подачу value proposition
- onboarding copy

### Как сделать лучше у себя

- использовать их простоту позиционирования
- но не повторять “маркетинг вместо архитектуры”

---

## 9. mainline-main

### Что это

Rust-реализация BitTorrent Mainline DHT.

### Плюсы

- написано на Rust
- хорошая репутация самой идеи Mainline DHT
- фокус на “fast time-to-first-response”
- можно использовать как inspiration для lightweight distributed discovery

### Минусы

- это DHT, а не NAT traversal solution
- не заточено под Minecraft
- не решает transport path само по себе

### Подходит нам?

Да, частично.

### Что подчерпнём

- лёгкий discovery подход на Rust
- распределённое хранение room/host metadata
- более сильную decentralization story на будущее

### Как сделать лучше у себя

- использовать DHT только после стабилизации transport layer
- сначала решить доставку трафика, потом уже decentralize lobby

---

## 10. matchbox-main

### Что это

Go-сервис для PXE/bare-metal provisioning.
К нашей задаче почти не относится.

### Плюсы

- хороший пример “service + API + provisioning logic”

### Минусы

- доменно почти не связан с P2P Minecraft
- ничего важного для NAT traversal, relay или localhost proxy не даёт

### Подходит нам?

Нет.

### Что подчерпнём

- почти ничего релевантного

### Как сделать лучше у себя

- просто не тратить на это время

---

## 11. MinecraftConnectTool-master

### Что это

Очень важный референс.
По README видно, что это Windows-инструмент для Minecraft-соединений, использующий:

- OpenP2P
- FRP / смешанный LinkMode
- P2PMode
- probe server
- диагностику NAT/соединения

По сути это один из ближайших к нам по product surface проектов.

### Плюсы

- очень близкий продуктовый сценарий
- уже есть:
  - P2P mode
  - relay/link mode
  - UI под Windows
  - настройки
  - логирование
- реальный пользовательский продукт, а не лабораторный код

### Минусы

- WinForms/.NET и специфичный локальный стек
- опирается на OpenP2P/FRP-экосистему
- часть архитектуры может быть завязана на конкретные внешние сервисы

### Подходит нам?

Да. Один из самых полезных проектов в наборе.

### Что подчерпнём

- UX поток host/join
- разделение `P2PMode` и `LinkMode`
- built-in diagnostics
- admin/settings/log panels
- логику “подключайся к 127.0.0.1:xxxxx”

### Как сделать лучше у себя

- сохранить их продуктовую понятность
- сделать более современную архитектуру на Rust + helper process
- убрать зависимость от конкретных закрытых сервисов

---

## 12. P2P-Minecraft-server-develop

### Что это

Исследовательский проект P2P Minecraft server architecture:

- client proxy
- server proxy
- spatial publish/subscribe
- spigot server

### Плюсы

- интересная академическая попытка
- есть важная идея прокси вокруг Minecraft
- показывает, что можно вынести Minecraft communication в отдельные proxy layers

### Минусы

- древний стек
- завязка на Minecraft 1.11.2
- сложная и тяжёлая схема запуска
- не выглядит как путь к современному consumer-grade продукту

### Подходит нам?

Частично, только как исторический/исследовательский reference.

### Что подчерпнём

- proxy-first взгляд на Minecraft transport
- идею разделения client proxy и server proxy

### Как сделать лучше у себя

- повторить proxy idea, но без старой многочастной адской схемы
- не тащить SPS как центральную модель продукта

---

## 13. p2pmcclient-main

### Что это

Fabric client mod для P2P Minecraft.
README говорит о трёх частях:

- client mod
- Purpur server/plugin logic
- FastAPI tracker/backend

### Плюсы

- очень близко к Minecraft domain
- показывает, как встроить P2P entrypoint прямо в Multiplayer UI
- интересен world-data / delta patching подход

### Минусы

- мод-ориентированная архитектура
- требует клиентских модификаций
- уводит нас от desktop-first connector без мода
- world-sync логика сильно шире нашей текущей цели

### Подходит нам?

Частично.

### Что подчерпнём

- UX-идею отдельного P2P screen
- то, как можно аккуратно объяснить пользователю P2P вход прямо в знакомом игровом сценарии

### Как сделать лучше у себя

- сделать тот же уровень простоты, но внешним приложением
- не лезть пока в delta-sync мира и plugin/mod экосистему

---

## Итоговые выводы

## Самые полезные проекты для нас

### `e4mc-minecraft-architectury-rererewrite`

Почему:

- closest match к боли LAN-over-Internet
- Minecraft-specific
- есть реально рабочая transport idea

Нужно взять:

- UX-модель
- LAN-to-public abstraction
- transport state thinking

### `MinecraftConnectTool-master`

Почему:

- closest match к desktop-коннектору
- уже есть P2P/link split
- есть diagnostics и practical Windows UX

Нужно взять:

- product flow
- diagnostics
- host/join state machine

### `ConnectX-main`

Почему:

- сильная сервисная архитектура
- room/server/relay/client split
- low-latency relay мышление

Нужно взять:

- server/relay/client decomposition
- control plane separation

### `mainline-main`

Почему:

- Rust
- хороший lightweight discovery reference

Нужно взять:

- future decentralized discovery ideas

---

## Что точно не надо брать как основу

- `matchbox-main`
- `Installer-main`
- `EOS-SDK-49960398-Release-v1.19.0.3`

Они не решают наш основной продуктовый вопрос.

---

## Рекомендованная новая стратегия для нашего продукта

Строить новый `Minecraft P2P Connector` на основе следующей смеси:

1. Product UX from `MinecraftConnectTool`
2. Minecraft LAN philosophy from `e4mc`
3. Service decomposition from `ConnectX`
4. Future discovery ideas from `mainline`
5. Helper-process architecture inspired by `kurai`
6. Lobby polish inspired by `voxel`

## Практически это означает

- Tauri UI отдельно
- networking helper отдельно
- direct transport first
- fallback transport second
- Minecraft always connects to `localhost`
- user never sees raw transport complexity

## Что сделать лучше конкурентов

Конкуренты обычно сыпятся в одном из двух мест:

- либо плохой UX
- либо хрупкий transport

Наше преимущество должно быть в комбинации:

- нормальный consumer UX
- честная диагностика
- transport ladder из нескольких уровней
- без ощущения “я запускаю какой-то VPN-костыль”

## Финальный вердикт

Если выбирать 4 проекта, которые реально нужно изучать глубже и превращать в инженерные решения, это:

1. `e4mc-minecraft-architectury-rererewrite`
2. `MinecraftConnectTool-master`
3. `ConnectX-main`
4. `mainline-main`

Именно они дают нам наилучшее сочетание:

- продуктовой близости
- архитектурной пользы
- применимости к нашей задаче
- реальной возможности сделать лучше, чем существующие решения
