<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Home, Settings } from 'lucide-vue-next'
import HomeView from './views/HomeView.vue'
import SettingsView from './views/SettingsView.vue'
import ProfileView from './views/ProfileView.vue'
import AuthModal from './components/AuthModal.vue'
import { supabase } from './supabase.js'
import { globalProfile } from './store.js'

// ── Navigation ──────────────────────────────────────────────
const activeTab = ref('home')

// ── Network state ────────────────────────────────────────────
const status = ref({ mode: 'idle', state: 'idle' })
const lobbyServers = ref([])

// ── Logs ─────────────────────────────────────────────────────
const logBuffer = ref([])
const MAX_LOGS = 120

function addLog(message) {
  const stamp = new Date().toLocaleTimeString('ru-RU', {
    hour: '2-digit', minute: '2-digit', second: '2-digit'
  })
  logBuffer.value.unshift(`[${stamp}] ${message}`)
  if (logBuffer.value.length > MAX_LOGS) logBuffer.value = logBuffer.value.slice(0, MAX_LOGS)
}

// ── Auth ──────────────────────────────────────────────────────
const user = ref(null)
const isGuest = ref(false)
const showAuthModal = ref(true)

const sidebarName = computed(() => {
  if (isGuest.value) return 'Гость'
  return user.value?.user_metadata?.full_name
    || user.value?.email?.split('@')[0]
    || globalProfile.nickname
})

const sidebarLetter = computed(() => sidebarName.value.slice(0, 1).toUpperCase())

// ── Timers & unlisten handles ─────────────────────────────────
let pollInterval = null
let lobbyInterval = null
let unlistenLobbyEvent = null
let unlistenConnectRequest = null
let unlistenPeerConnected = null
let unlistenPeerDisconnected = null

// ── Incoming connection modal ──────────────────────────────────
const incomingRequest = ref(null)  // { peerId, peerName, serverId }
const currentHostPassword = ref(null)

// ── Lifecycle ─────────────────────────────────────────────────
onMounted(async () => {
  // Apply saved theme immediately
  const savedTheme = localStorage.getItem('minecraft-p2p-theme') || 'oled'
  document.body.dataset.theme = savedTheme
  const savedAccent = localStorage.getItem('minecraft-p2p-accent') || 'blue'
  document.documentElement.style.setProperty('--accent', `var(--accent-${savedAccent})`)
  document.documentElement.style.setProperty('--accent-strong', `color-mix(in srgb, var(--accent-${savedAccent}) 80%, white)`)

  // Poll status every 2s
  pollInterval = setInterval(async () => {
    try {
      const s = await invoke('get_status')
      status.value = s
      // Merge Rust logs
      if (s?.logs?.length) {
        s.logs.forEach(line => {
          if (!logBuffer.value.find(l => l.endsWith(line))) addLog(line)
        })
      }
    } catch (e) {
      console.error('get_status failed:', e)
    }
  }, 2000)

  // Poll lobby every 10s
  lobbyInterval = setInterval(async () => {
    try {
      lobbyServers.value = await invoke('refresh_lobby')
    } catch (e) {
      console.error('refresh_lobby failed:', e)
    }
  }, 10000)

  // Initial fetch
  invoke('get_status').then(s => { status.value = s }).catch(console.error)
  invoke('refresh_lobby').then(r => { lobbyServers.value = r }).catch(console.error)

  // Tauri event: lobby-event — обрабатывает все входящие Ably события
  // Rust эмитит ВСЕ SSE сообщения через 'lobby-event' с полем data (Ably message)
  try {
    unlistenLobbyEvent = await listen('lobby-event', (event) => {
      const raw = event.payload           // { channel, data }
      const ablyMsg = raw?.data || {}     // Ably message: { name, data, clientId, ... }
      const evName = ablyMsg?.name || ''

      // Ably кодирует поле data как JSON-строку — парсим
      let innerData = ablyMsg?.data
      if (typeof innerData === 'string') {
        try { innerData = JSON.parse(innerData) } catch (_) {}
      }
      
      if (evName === 'connect-request') {
        // Входящий запрос на подключение к нашему серверу (Desktop хост)
        const payload = innerData || {}
        incomingRequest.value = payload
        addLog(`Входящий запрос от ${payload?.client_id || payload?.peerId || 'unknown'}`)
        
        if (currentHostPassword.value) {
          if (payload?.password === currentHostPassword.value) {
            addLog('Пароль верный, авто-принятие.')
            acceptRequest()
          } else {
            addLog('Неверный пароль, отклонено.')
            declineRequest()
          }
        } else {
          acceptRequest()
        }
      } else if (evName === 'connect-ack') {
        addLog(`[Lobby] connect-ack получен`)
        // Обрабатывается в HomeView через отдельный listen('lobby-event')
      } else if (evName === 'host-presence' || evName === '') {
        // Новый хост появился — обновляем лобби
        invoke('refresh_lobby').then(r => { lobbyServers.value = r }).catch(() => {})
      } else {
        addLog(`[Lobby] ${evName} на ${raw?.channel || '?'}`)
      }
    })
  } catch (e) { console.warn('lobby-event listener failed:', e) }

  // NOTE: 'connect-request' direct listener удалён — Rust эмитит всё через 'lobby-event'
  unlistenConnectRequest = () => {} // no-op для совместимости onUnmounted


  // Tauri event: peer-connected
  try {
    unlistenPeerConnected = await listen('peer-connected', (event) => {
      addLog(`Игрок подключился: ${event.payload?.peerId || 'unknown'}`)
    })
  } catch (e) { console.warn('peer-connected listener failed:', e) }

  // Tauri event: peer-disconnected
  try {
    unlistenPeerDisconnected = await listen('peer-disconnected', (event) => {
      addLog(`Игрок отключился: ${event.payload?.peerId || 'unknown'}`)
    })
  } catch (e) { console.warn('peer-disconnected listener failed:', e) }

  // Auth
  const { data: { session } } = await supabase.auth.getSession()
  if (session) {
    user.value = session.user
    showAuthModal.value = false
  }

  supabase.auth.onAuthStateChange((_event, session) => {
    if (session) {
      user.value = session.user
      showAuthModal.value = false
    } else {
      user.value = null
      if (!isGuest.value) showAuthModal.value = true
    }
  })
})

onUnmounted(() => {
  clearInterval(pollInterval)
  clearInterval(lobbyInterval)
  unlistenLobbyEvent?.()
  unlistenConnectRequest?.()
  unlistenPeerConnected?.()
  unlistenPeerDisconnected?.()
})

// ── Auth handlers ─────────────────────────────────────────────
const handleLogin = async (credentials) => {
  try {
    if (credentials.isRegister) {
      const { error } = await supabase.auth.signUp({
        email: credentials.email,
        password: credentials.password
      })
      if (error) throw error
      alert('Регистрация успешна! Войдите.')
    } else {
      const { error } = await supabase.auth.signInWithPassword({
        email: credentials.email,
        password: credentials.password
      })
      if (error) throw error
    }
  } catch (err) {
    alert('Ошибка: ' + err.message)
  }
}

const handleGuest = () => {
  isGuest.value = true
  showAuthModal.value = false
  addLog('Вход как гость')
}

const handleGoogle = async () => {
  await supabase.auth.signInWithOAuth({ provider: 'google' })
}

const handleLogout = async () => {
  await supabase.auth.signOut()
  isGuest.value = false
  addLog('Выход из аккаунта')
}

// ── Incoming connect request handlers ─────────────────────────
const acceptRequest = async () => {
  if (!incomingRequest.value) return
  try {
    const peerId = incomingRequest.value.client_id || incomingRequest.value.peerId
    const peerAddr = incomingRequest.value.peer_addr || ''
    const relaySessionId = incomingRequest.value.relay_session_id || null

    // Отправляем connect-ack клиенту
    await invoke('publish_lobby_event', {
      channel: `lobby:${peerId}`,
      event: 'connect-ack',
      payload: {
        relay_session_id: relaySessionId,
        host_id: 'desktop-host',
        accepted: true
      }
    })
    
    // Открываем P2P туннель к клиенту
    if (peerAddr && peerAddr !== '0.0.0.0:0' && peerAddr !== '') {
      await invoke('connect_to_peer', {
        peerId: peerId,
        peerAddrs: [peerAddr],
        relaySessionId: relaySessionId
      })
    }

    addLog(`Принято подключение от ${peerId}`)
  } catch (e) {
    addLog('Ошибка принятия: ' + e)
  } finally {
    incomingRequest.value = null
  }
}

const declineRequest = async () => {
  if (!incomingRequest.value) return
  try {
    const peerId = incomingRequest.value.client_id || incomingRequest.value.peerId;
    await invoke('publish_lobby_event', {
      channel: `lobby:${peerId}`,
      event: 'connect-ack',
      payload: {
        peerId: status.value?.peers?.[0]?.peer_id || 'host',
        accepted: false
      }
    })
    addLog(`Отклонено подключение от ${incomingRequest.value.peerName || peerId}`)
  } catch (e) { console.error(e) }
  finally { incomingRequest.value = null }
}
</script>

<template>
  <div class="app-shell" data-theme="oled">
    <!-- Sidebar -->
    <aside class="sidebar">
      <div class="sidebar-brand-block">
        <button class="brand-mark avatar-trigger" type="button" @click="activeTab = 'profile'">
          <img
            v-if="user?.user_metadata?.avatar_url || globalProfile.avatarDataUrl"
            :src="globalProfile.avatarDataUrl || user?.user_metadata?.avatar_url"
            class="brand-avatar-image"
            alt=""
            @error="(e) => e.target.style.display = 'none'"
          />
          <span v-else id="brand-avatar-fallback">{{ sidebarLetter }}</span>
        </button>
        <div class="brand-copy">
          <span class="brand-label">Minecraft P2P Connector</span>
          <strong>{{ sidebarName }}</strong>
        </div>
      </div>

      <nav class="sidebar-nav">
        <button
          class="nav-button"
          :class="{ 'nav-button-active': activeTab === 'home' }"
          @click="activeTab = 'home'"
          aria-label="Главная"
        >
          <Home :size="24" />
          <span>Главная</span>
        </button>
        <button
          class="nav-button"
          :class="{ 'nav-button-active': activeTab === 'settings' }"
          @click="activeTab = 'settings'"
          aria-label="Настройки"
        >
          <Settings :size="24" />
          <span>Настройки</span>
        </button>
      </nav>
    </aside>

    <!-- Main workspace -->
    <main class="workspace">
      <HomeView
        v-if="activeTab === 'home'"
        :status="status"
        :servers="lobbyServers"
        @refresh="invoke('refresh_lobby').then(r => { lobbyServers.value = r }).catch(console.error)"
        @host-started="currentHostPassword = $event"
      />
      <SettingsView
        v-if="activeTab === 'settings'"
        :logs="logBuffer"
        :user="user"
        :isGuest="isGuest"
        @logout="handleLogout"
      />
      <ProfileView
        v-if="activeTab === 'profile'"
        :user="user"
        :isGuest="isGuest"
        @logout="handleLogout"
      />
      </main>

      <!-- Auth modal -->
    <AuthModal
      :isOpen="showAuthModal"
      @login="handleLogin"
      @guest="handleGuest"
      @google="handleGoogle"
    />

    <!-- Incoming connect request toast -->
    <Transition name="slide-up">
      <div v-if="incomingRequest" class="incoming-request-toast">
        <div class="incoming-request-body">
          <div class="incoming-avatar">{{ (incomingRequest.peerName || '?').slice(0, 1).toUpperCase() }}</div>
          <div class="incoming-text">
            <strong>{{ incomingRequest.peerName || incomingRequest.peerId }}</strong>
            <span>хочет подключиться к вашему серверу</span>
          </div>
        </div>
        <div class="incoming-actions">
          <button class="primary-button" style="padding: 6px 16px; font-size: 13px;" @click="acceptRequest">Принять</button>
          <button class="ghost-button danger-button" style="padding: 6px 14px; font-size: 13px;" @click="declineRequest">Отклонить</button>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
/* Incoming request toast */
.incoming-request-toast {
  position: fixed;
  bottom: 24px;
  right: 24px;
  z-index: 200;
  background: var(--surface-raised);
  border: 1px solid var(--line);
  border-radius: var(--radius);
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-width: 280px;
  max-width: 360px;
  box-shadow: 0 8px 32px rgba(0,0,0,0.4);
}

.incoming-request-body {
  display: flex;
  align-items: center;
  gap: 12px;
}

.incoming-avatar {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  background: var(--accent);
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 700;
  font-size: 15px;
  flex-shrink: 0;
}

.incoming-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.incoming-text strong {
  font-size: 14px;
  color: var(--text-base);
}

.incoming-text span {
  font-size: 12px;
  color: var(--text-soft);
}

.incoming-actions {
  display: flex;
  gap: 8px;
}

.brand-avatar-image {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: inherit;
}

/* Toast animation */
.slide-up-enter-active, .slide-up-leave-active {
  transition: all 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.slide-up-enter-from, .slide-up-leave-to {
  opacity: 0;
  transform: translateY(20px) scale(0.95);
}
</style>
