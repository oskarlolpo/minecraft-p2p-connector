<script setup>
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import HostModal from '../components/HostModal.vue'
import ConnectModal from '../components/ConnectModal.vue'
import PeerList from '../components/PeerList.vue'

const props = defineProps({
  status: Object,
  servers: { type: Array, default: () => [] }
})
const emit = defineEmits(['refresh'])

const isHostModalOpen = ref(false)
const connectModal = ref({ open: false, server: null })
const searchQuery = ref('')
const isRefreshing = ref(false)

const filteredServers = computed(() => {
  const q = searchQuery.value.toLowerCase().trim()
  if (!q) return props.servers
  return props.servers.filter(s =>
    (s.room_name || s.title || s.roomName || '').toLowerCase().includes(q) ||
    (s.host_name || s.nickname || '').toLowerCase().includes(q)
  )
})

const peers = computed(() => props.status?.peers ?? [])

const isHosting = computed(() => props.status?.mode === 'host')
const isClient = computed(() => props.status?.mode === 'client')
const isActive = computed(() => isHosting.value || isClient.value)

const stateLabel = computed(() => {
  const map = {
    idle: 'Ожидание',
    starting: 'Запуск...',
    hosting: 'Хостинг',
    connected: 'Подключено',
    connecting: 'Подключение...',
    waitingForPeer: 'Ожидание игрока',
    punching: 'NAT Punching',
    error: 'Ошибка',
  }
  return map[props.status?.state] ?? props.status?.state ?? 'Ожидание'
})

const handleStartHost = async (config) => {
  try {
    await invoke('start_hosting', {
      roomName: config.roomName,
      roomTheme: config.roomTheme,
      roomPassword: config.roomPassword,
      requirePassword: config.requirePassword,
      gameVersion: config.gameVersion,
      localPort: config.localGamePort,
      isExternalHost: config.isExternalHost,
      externalHostAddress: config.externalHostAddress,
      forceDirectMode: config.forceDirectMode,
      enableGeyser: config.enableGeyser,
      geyserPort: config.geyserPort,
      enableE4mc: false,
    })
    emit('host-started', config.password || null)
    isHostModalOpen.value = false
  } catch (e) {
    alert('Ошибка запуска: ' + e)
  }
}

const stopHosting = async () => {
  try {
    await invoke('stop_hosting')
  } catch (e) {
    console.error(e)
  }
}

const openConnectModal = (server) => {
  connectModal.value = { open: true, server }
}

const handleConnect = async ({ password }) => {
  const s = connectModal.value.server
  if (!s) return
  try {
    // Собираем все адреса (публичный + локальные) — Rust сам выберет лучший
    const addrs = [
      s.public_join_address || s.endpoint || s.serverIp || s.server_ip,
      ...(s.local_ip ? s.local_ip.split(',').map(a => a.trim()).filter(Boolean) : [])
    ].filter(Boolean)

    if (addrs.length === 0) {
      alert('Нет адресов для подключения')
      return
    }

    await invoke('prepare_client_connect', {
      peerId: s.client_id || s.peer_id || s.id || '',
      peerAddrs: addrs,
      roomName: s.room_name || s.title || '',
      hostName: s.host_name || s.nickname || '',
      mcVersion: s.minecraft_version || s.version || s.mcVer || '',
      slots: s.slots || s.players || '1/30'
    })
    
    // 2. Получаем наш публичный UDP адрес после STUN
    const currentStatus = await invoke('get_status')
    const myUdpAddr = currentStatus?.publicUdpAddr || currentStatus?.udpBindAddr || ''
    
    // 3. Генерируем ID этой сессии
    const myClientId = 'desktop-client-' + Math.floor(Math.random() * 100000)
    const relaySessionId = (typeof crypto !== 'undefined' && crypto.randomUUID)
      ? crypto.randomUUID()
      : ('relay-' + Date.now())

    // 4. Подписываемся на ответ хоста (наш личный канал)
    invoke('subscribe_lobby_events', { channel: `lobby:${myClientId}` }).catch(() => {})
    
    // 5. Ждём connect-ack через tauri event
    const ackPromise = new Promise((resolve) => {
      let unlisten = null
      listen('lobby-event', (event) => {
        try {
          const raw = event.payload              // { channel, data }
          const ablyMsg = raw?.data || {}        // Ably message: { name, data }
          const evName = ablyMsg?.name || ''
          if (evName !== 'connect-ack') return
          
          // Ably data может быть JSON-строкой
          let innerData = ablyMsg?.data
          if (typeof innerData === 'string') {
            try { innerData = JSON.parse(innerData) } catch (_) {}
          }
          const data = innerData || {}
          
          if (unlisten) unlisten()
          resolve(data)
        } catch (_) {}
      }).then(fn => { unlisten = fn })
      setTimeout(() => {
        if (unlisten) unlisten()
        resolve(null)
      }, 12000)
    })

    // 6. Отправляем connect-request хосту
    await invoke('publish_lobby_event', {
      channel: `lobby:${s.client_id || s.peer_id || s.id}`,
      event: 'connect-request',
      payload: {
        client_id: myClientId,
        peer_addr: myUdpAddr,
        relay_session_id: relaySessionId,
        password: password || null
      }
    })
    
    // 7. Ждём ответа хоста
    const ack = await ackPromise
    
    if (ack && ack.accepted === true) {
      // 8. Открываем QUIC туннель
      await invoke('commit_prepared_client_connect', {
        relaySessionId: relaySessionId,
        useUdp: true
      })
    } else if (ack && ack.accepted === false) {
      alert('Хост отклонил запрос (неверный пароль или занято)')
    } else {
      // Таймаут — хост возможно не Desktop (Android не посылает ack), всё равно коммитим
      await invoke('commit_prepared_client_connect', {
        relaySessionId: relaySessionId,
        useUdp: true
      })
    }
    
    connectModal.value = { open: false, server: null }
  } catch (e) {
    alert('Ошибка подключения: ' + e)
  }
}

const refreshLobby = async () => {
  isRefreshing.value = true
  try {
    await invoke('refresh_lobby')
    emit('refresh')
  } catch (e) {
    console.error(e)
  } finally {
    isRefreshing.value = false
  }
}

const copyEndpoint = (text) => {
  navigator.clipboard.writeText(text).catch(() => {})
}
</script>

<template>
  <section class="page page-active">
    <!-- Hero -->
    <header class="hero-panel">
      <div class="hero-left">
        <h1>Minecraft P2P Connector</h1>
        <div class="hero-actions">
          <button class="primary-button" type="button" @click="isHostModalOpen = true" :disabled="isActive">
            Создать хост
          </button>
          <button class="ghost-button" type="button" @click="refreshLobby" :disabled="isRefreshing" style="display:flex;align-items:center;gap:6px;">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" :style="isRefreshing ? 'animation:spin 0.8s linear infinite' : ''">
              <polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
            </svg>
            Обновить лобби
          </button>
        </div>
      </div>
      <div class="hero-right">
        <div class="status-orb">
          <span class="status-label">Network Status</span>
          <strong>{{ stateLabel }}</strong>
          <span>{{ status?.mode ?? 'idle' }}</span>
        </div>
        <p class="hero-note">
          <template v-if="isHosting">
            Ваш сервер опубликован. Порт: <strong>{{ status?.localGamePort ?? '—' }}</strong>
          </template>
          <template v-else-if="isClient">
            Вы подключены к серверу.
          </template>
          <template v-else>
            Запустите хост, чтобы опубликовать локальный мир или сервер.
          </template>
        </p>
      </div>
    </header>

    <div class="content-grid">
      <div class="stack-column" style="width: 100%;">

        <!-- Active session panel -->
        <section v-if="isActive" class="panel panel-large" id="active-session-panel">
          <div class="panel-head">
            <div>
              <h2>{{ isHosting ? 'Активная сессия (Хост)' : 'Активная сессия (Клиент)' }}</h2>
            </div>
            <button class="ghost-button danger-button" @click="stopHosting">Остановить</button>
          </div>

          <!-- Host info card -->
          <div v-if="isHosting && status" class="active-host-card">
            <div class="host-info-row">
              <span>Публичный адрес</span>
              <strong style="cursor: pointer;" @click="copyEndpoint(status.publicUdpAddr || status.udpBindAddr || '')">
                {{ status.publicUdpAddr || status.udpBindAddr || 'Определяется...' }}
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="opacity:0.5"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
              </strong>
            </div>
            <div class="host-info-row" v-if="status.bedrockPort">
              <span>Bedrock порт</span>
              <strong>{{ status.bedrockPort }}</strong>
            </div>
            <div class="host-info-row">
              <span>Игроков</span>
              <strong>{{ peers.length }}/{{ status.maxPlayers ?? 30 }}</strong>
            </div>
          </div>

          <!-- Peer list -->
          <PeerList :peers="peers" :status="status" />
        </section>

        <!-- Server search -->
        <div class="server-list-controls" style="display: flex; gap: 12px; margin-bottom: 16px; align-items: center;">
          <input
            v-model="searchQuery"
            type="text"
            placeholder="Поиск серверов..."
            style="flex: 1; padding: 12px; border-radius: 8px; border: 1px solid var(--line); background: var(--surface-raised); color: var(--text-base); font-size: 14px;"
          />
        </div>

        <!-- Lobby list -->
        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Список серверов</span>
              <h2>{{ filteredServers.length }} сервер(ов)</h2>
            </div>
          </div>
          <div class="row-head server-head">
            <span>Сервер</span>
            <span>Версия</span>
            <span>Онлайн</span>
            <span>Действие</span>
          </div>
          <div class="server-list">
            <div
              v-if="filteredServers.length === 0"
              style="padding: 40px 24px; text-align: center; color: var(--text-muted);"
            >
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="opacity:0.3; margin-bottom: 8px; display: block; margin: 0 auto 8px;"><rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/></svg>
              Лобби пусто
            </div>
            <div v-else v-for="s in filteredServers" :key="s.client_id || s.id || s.peer_id || Math.random()" class="server-row">
              <div class="server-info">
                <div class="server-name">{{ s.room_name || s.title || s.roomName || 'Unnamed' }}</div>
                <div class="server-host">{{ s.host_name || s.nickname || 'Unknown' }} · {{ s.minecraft_version || s.mcVer || s.version || 'Java' }}</div>
              </div>
              <div class="server-version">{{ s.minecraft_version || s.mcVer || s.version || '—' }}</div>
              <div class="server-players">{{ s.slots || s.players || s.playerCount || 0 }}/{{ s.players_max || s.maxPlayers || '?' }}</div>
              <button class="primary-button" @click="openConnectModal(s)" :disabled="isClient">
                Подключиться
              </button>
            </div>
          </div>
        </section>
      </div>
    </div>

    <!-- Modals -->
    <HostModal
      :isOpen="isHostModalOpen"
      @close="isHostModalOpen = false"
      @start-host="handleStartHost"
    />
    <ConnectModal
      :isOpen="connectModal.open"
      :serverName="connectModal.server?.room_name"
      :serverHost="connectModal.server?.host_name"
      :hasPassword="connectModal.server?.has_password"
      @close="connectModal = { open: false, server: null }"
      @connect="handleConnect"
    />
  </section>
</template>
