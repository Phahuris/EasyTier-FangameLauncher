<script setup lang="ts">
import { type } from '@tauri-apps/plugin-os'
import { invoke } from '@tauri-apps/api/core'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { open } from '@tauri-apps/plugin-shell'
import { exit } from '@tauri-apps/plugin-process'
import { I18nUtils, RemoteManagement, Utils } from "easytier-frontend-lib"
import type { MenuItem } from 'primevue/menuitem'
import { useTray } from '~/composables/tray'
import { initMobileVpnService } from '~/composables/mobile_vpn'
import { GUIRemoteClient } from '~/modules/api'
import { useToast, useConfirm } from 'primevue'
import { loadMode, saveMode, WebClientConfig, type Mode } from '~/composables/mode'
import { saveLastNetworkInstanceId, loadLastNetworkInstanceId } from '~/composables/config'
import ModeSwitcher from '~/components/ModeSwitcher.vue'
import { getEasytierVersion, getServiceStatus } from '~/composables/backend'

const { t } = useI18n()
const confirm = useConfirm()
const aboutVisible = ref(false)
const modeDialogVisible = ref(false)
const currentMode = ref<Mode>({ mode: 'normal' })
const editingMode = ref<Mode>({ mode: 'normal' })
const isModeSaving = ref(false)
const manualDisconnect = ref(false)
const configServerDialogVisible = ref(false)
const configServerConnected = ref(false)

const activeTab = ref<'create' | 'join'>('create')
const pseudo = ref(localStorage.getItem('fgl_pseudo') || '')
const chatInput = ref('')
const logLines = ref<string[]>([])
const isBusy = ref(false)
const showAdvanced = ref(false)

// Champs EasyTier simplifiés (Créer)
const hostNetworkName = ref(localStorage.getItem('fgl_net_name') || 'fangame')
const hostNetworkSecret = ref(localStorage.getItem('fgl_net_secret') || '')
const hostStatus = ref('')
const hostShareCode = ref('')

// Champs EasyTier simplifiés (Rejoindre)
const joinNetworkName = ref(localStorage.getItem('fgl_net_name') || 'fangame')
const joinNetworkSecret = ref(localStorage.getItem('fgl_net_secret') || '')
const joinPeerUrl = ref('')
const joinStatus = ref('')

watch(pseudo, (v) => localStorage.setItem('fgl_pseudo', v))
watch(hostNetworkName, (v) => {
  localStorage.setItem('fgl_net_name', v)
  joinNetworkName.value = v
})
watch(hostNetworkSecret, (v) => {
  localStorage.setItem('fgl_net_secret', v)
  joinNetworkSecret.value = v
})

function addLog(msg: string) {
  const ts = new Date().toLocaleTimeString()
  logLines.value.push(`[${ts}] ${msg}`)
  if (logLines.value.length > 400) logLines.value.shift()
  nextTick(() => {
    const el = document.getElementById('fgl-logbox')
    if (el) el.scrollTop = el.scrollHeight
  })
}

function copyShareCode() {
  const code = hostShareCode.value
  if (!code) return
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(code).then(() => addLog('Code copie')).catch(() => addLog('Copie impossible'))
  } else {
    addLog('Code: ' + code)
  }
}

async function sendChat() {
  const msg = chatInput.value.trim()

  if (!msg) {
    return
  }

  const name = pseudo.value.trim() || 'Anonyme'

  chatInput.value = ''

  addLog(`${name} : ${msg}`)

  try {
    await invoke('chat_start')

    /*
     * Récupération des informations EasyTier.
     * Les peers contiennent leurs IPv4 virtuelles.
     */
    const ids = await invoke<any[]>('list_network_instance_ids')

    let peers: string[] = []

    for (const id of ids || []) {
      try {
        const info = await invoke<any>('collect_network_info', {
          inst_id: id
        })

        const network = info?.info?.map?.[id]

        if (!network) {
          continue
        }

        const myIp =
          network?.my_node_info?.virtual_ipv4 ||
          network?.my_node_info?.virtual_ip

        const found: string[] = []

        for (const peer of network?.peers || []) {
          const ip =
            peer?.virtual_ipv4 ||
            peer?.virtual_ip ||
            peer?.ipv4_addr ||
            peer?.ip

          if (ip && ip !== myIp) {
            found.push(String(ip).split('/')[0])
          }
        }

        peers.push(...found)
      }
      catch (e) {
        console.warn('Impossible de lire les peers EasyTier:', e)
      }
    }

    peers = [...new Set(peers)]

    console.log('[CHAT] Peers EasyTier:', peers)

    if (peers.length === 0) {
      addLog('[Chat] Aucun joueur EasyTier trouvé')
      return
    }

    await invoke('chat_send', {
      pseudo: name,
      text: msg,
      peers
    })
  }
  catch (e) {
    addLog('Chat réseau EasyTier: ' + String(e))
    console.error('[CHAT]', e)
  }
}

const uiLang = ref(localStorage.getItem('lang') || 'fr')
const uiStrings: Record<string, Record<string, string>> = {
  fr: {
    title: 'FANGAMELAUNCHER',
    pseudo: 'Ton pseudo',
    create: 'Créer une partie',
    join: 'Rejoindre une partie',
    netName: 'Nom du réseau',
    netSecret: 'Mot de passe réseau (secret)',
    peerUrl: 'Adresse du serveur (peer)',
    peerPh: 'ex: tcp://IP:11010',
    startHost: 'Démarrer la partie',
    stopHost: 'Arrêter',
    doJoin: 'Rejoindre',
    language: 'Langue',
    logsChat: 'Logs / Chat',
    send: 'Envoyer',
    advanced: 'Options avancées EasyTier',
    needPseudo: 'Pseudo obligatoire.',
    needName: 'Nom du réseau obligatoire.',
    needPeer: 'Adresse du serveur obligatoire.',
    noClient: 'Client EasyTier non connecté (il faut le backend Tauri / easytier-core).',
    hostOk: 'Réseau host démarré.',
    joinOk: 'Connexion au réseau lancée.',
    hostRunning: 'Partie active (host)',
    noParty: 'Aucune partie',
  },
  en: {
    title: 'FANGAMELAUNCHER',
    pseudo: 'Nickname',
    create: 'Create party',
    join: 'Join party',
    netName: 'Network name',
    netSecret: 'Network password (secret)',
    peerUrl: 'Server address (peer)',
    peerPh: 'e.g. tcp://IP:11010',
    startHost: 'Start party',
    stopHost: 'Stop',
    doJoin: 'Join',
    language: 'Language',
    logsChat: 'Logs / Chat',
    send: 'Send',
    advanced: 'Advanced EasyTier options',
    needPseudo: 'Nickname required.',
    needName: 'Network name required.',
    needPeer: 'Server address required.',
    noClient: 'EasyTier client not connected (need Tauri backend / easytier-core).',
    hostOk: 'Host network started.',
    joinOk: 'Join network started.',
    hostRunning: 'Party active (host)',
    noParty: 'No party',
  },
}
const s = computed(() => uiStrings[uiLang.value] || uiStrings.fr)

async function setLanguage(lang: string) {
  uiLang.value = lang
  localStorage.setItem('lang', lang)
  try { await I18nUtils.loadLanguageAsync('en') } catch (e) { console.error(e) }
  addLog(lang === 'fr' ? 'Langue : Français' : 'Language : English')
}

function requirePseudo(): boolean {
  if (!pseudo.value.trim()) {
    addLog(s.value.needPseudo)
    return false
  }
  return true
}

function newId() {
  return crypto.randomUUID()
}

function buildHostConfig(): any {
  return {
    instance_id: newId(),
    dhcp: true,
    virtual_ipv4: '',
    network_length: 24,
    hostname: pseudo.value.trim(),
    network_name: hostNetworkName.value.trim() || 'fangame',
    network_secret: hostNetworkSecret.value,
    networking_method: 1,
    public_server_url: '',
    peer_urls: [],
    proxy_cidrs: [],
    enable_vpn_portal: false,
    vpn_portal_listen_port: 22022,
    vpn_portal_client_network_addr: '',
    vpn_portal_client_network_len: 24,
    advanced_settings: false,
    listener_urls: ['tcp://0.0.0.0:11010', 'udp://0.0.0.0:11010', 'wg://0.0.0.0:11011'],
    latency_first: false,
    dev_name: '',
    multi_thread: true,
    bind_device: true,
    enable_manual_routes: false,
    routes: [],
    exit_nodes: [],
    socks5_port: 1080,
    mtu: null,
    instance_recv_bps_limit: null,
    mapped_listeners: [],
    port_forwards: [],
    relay_network_whitelist: [],
  }
}

function buildJoinConfig(): any {
  const peer = joinPeerUrl.value.trim()
  return {
    instance_id: newId(),
    dhcp: true,
    virtual_ipv4: '',
    network_length: 24,
    hostname: pseudo.value.trim(),
    network_name: joinNetworkName.value.trim() || 'fangame',
    network_secret: joinNetworkSecret.value,
    networking_method: 1,
    public_server_url: '',
    peer_urls: peer ? [peer] : [],
    proxy_cidrs: [],
    enable_vpn_portal: false,
    vpn_portal_listen_port: 22022,
    vpn_portal_client_network_addr: '',
    vpn_portal_client_network_len: 24,
    advanced_settings: false,
    listener_urls: [],
    latency_first: false,
    dev_name: '',
    multi_thread: true,
    bind_device: true,
    enable_manual_routes: false,
    routes: [],
    exit_nodes: [],
    socks5_port: 1080,
    mtu: null,
    instance_recv_bps_limit: null,
    mapped_listeners: [],
    port_forwards: [],
    relay_network_whitelist: [],
  }
}

async function startHost() {
  if (!requirePseudo()) return
  if (!hostNetworkName.value.trim()) { addLog(s.value.needName); return }

  isBusy.value = true
  try {
    if (!clientRunning.value) {
      // MODE DEMO (preview navigateur sans Tauri)
      hostShareCode.value = 'tcp://127.0.0.1:11010'
      hostStatus.value = 'DEMO — partie simulee (pas de backend)'
      addLog('MODE DEMO : backend EasyTier absent (preview web).')
      addLog('Nom reseau: ' + hostNetworkName.value)
      addLog('Secret: ' + (hostNetworkSecret.value ? '(defini)' : '(vide)'))
      addLog('Pseudo host: ' + pseudo.value.trim())
      addLog('Code a partager (exemple): ' + hostShareCode.value)
      addLog('Sur un vrai client Tauri, les listeners tcp/udp:11010 + wg:11011 seraient actifs.')
      return
    }
    const cfg = buildHostConfig()
    await remoteClient.value.run_network(cfg, true)
    instanceId.value = cfg.instance_id
    hostStatus.value = s.value.hostRunning
    hostShareCode.value = 'tcp://127.0.0.1:11010'
    addLog(s.value.hostOk)
    addLog('Reseau: ' + cfg.network_name + ' | id: ' + cfg.instance_id)
    addLog('Code a partager: ' + hostShareCode.value)
  } catch (e: unknown) {
    addLog('Erreur host: ' + String(e))
    hostStatus.value = 'Erreur'
  } finally {
    isBusy.value = false
  }
}

async function stopHost() {
  if (!instanceId.value || !clientRunning.value) return
  isBusy.value = true
  try {
    await remoteClient.value.update_network_instance_state(instanceId.value, true)
    hostShareCode.value = ''
    hostStatus.value = s.value.noParty
    addLog('Host arrêté.')
  } catch (e: any) {
    addLog('Erreur stop: ' + (e?.message || String(e)))
  } finally {
    isBusy.value = false
  }
}

async function startJoin() {
  if (!requirePseudo()) return
  if (!joinNetworkName.value.trim()) { addLog(s.value.needName); return }
  if (!joinPeerUrl.value.trim()) { addLog(s.value.needPeer); return }

  isBusy.value = true
  try {
    if (!clientRunning.value) {
      joinStatus.value = 'DEMO — join simule (pas de backend)'
      addLog('MODE DEMO JOIN')
      addLog('Pseudo: ' + pseudo.value.trim())
      addLog('Reseau: ' + joinNetworkName.value)
      addLog('Peer: ' + joinPeerUrl.value)
      addLog('Sur Tauri, run_network serait appele avec peer_urls.')
      return
    }
    const cfg = buildJoinConfig()
    await remoteClient.value.run_network(cfg, true)
    instanceId.value = cfg.instance_id
    joinStatus.value = 'Connecte...'
    addLog(s.value.joinOk)
    addLog('Peer: ' + joinPeerUrl.value)
  } catch (e: unknown) {
    addLog('Erreur join: ' + String(e))
    joinStatus.value = 'Erreur'
  } finally {
    isBusy.value = false
  }
}

// ===== EasyTier mode / RPC (conservé) =====
async function openModeDialog() {
  editingMode.value = JSON.parse(JSON.stringify(loadMode()))
  modeDialogVisible.value = true
}

async function onModeSave() {
  if (isModeSaving.value) return
  isModeSaving.value = true
  try {
    await initWithMode(editingMode.value)
    modeDialogVisible.value = false
    addLog('Mode sauvegardé')
  } catch (e: any) {
    toast.add({ severity: 'error', summary: t('error'), detail: e, life: 10000 })
    await initWithMode(currentMode.value)
  } finally {
    isModeSaving.value = false
  }
}

async function onUninstallService() {
  confirm.require({
    message: t('mode.uninstall_service_confirm'),
    header: t('mode.uninstall_service'),
    icon: 'pi pi-exclamation-triangle',
    rejectProps: { label: t('web.common.cancel'), severity: 'secondary', outlined: true },
    acceptProps: { label: t('mode.uninstall_service'), severity: 'danger' },
    accept: async () => {
      isModeSaving.value = true
      try {
        await initWithMode({ ...currentMode.value, mode: 'normal' })
        await initService(undefined)
        modeDialogVisible.value = false
      } catch (e: any) {
        toast.add({ severity: 'error', summary: t('error'), detail: e, life: 10000 })
      } finally {
        isModeSaving.value = false
      }
    },
  })
}

function stripModeMetadata(mode: Mode) {
  if (mode.mode !== 'service') return mode
  const serviceConfig = { ...mode }
  delete serviceConfig.installed_core_version
  return serviceConfig
}
function modeConfigChanged(next: Mode) {
  return JSON.stringify(stripModeMetadata(next)) !== JSON.stringify(stripModeMetadata(currentMode.value))
}

async function onStopService() {
  isModeSaving.value = true
  manualDisconnect.value = true
  try {
    await setServiceStatus(false)
    modeDialogVisible.value = false
  } catch (e: any) {
    toast.add({ severity: 'error', summary: t('error'), detail: e, life: 10000 })
  } finally {
    isModeSaving.value = false
  }
}

async function initWithMode(mode: Mode) {
  const running_inst_ids = (await remoteClient.value.list_network_instance_ids().catch(() => undefined))?.running_inst_ids ?? []

  if (currentMode.value.mode === 'service' && mode.mode !== 'service') {
    let serviceStatus = await getServiceStatus()
    if (serviceStatus === "Running") {
      manualDisconnect.value = true
      await setServiceStatus(false)
      serviceStatus = await getServiceStatus()
      for (let i = 0; i < 10; i++) {
        if (serviceStatus === "Stopped") break
        await new Promise(resolve => setTimeout(resolve, 100))
        serviceStatus = await getServiceStatus()
      }
    }
    if (serviceStatus === "Stopped") await initService(undefined)
  }

  let url: string | undefined = undefined
  let retrys = 1
  switch (mode.mode) {
    case 'remote':
      if (!mode.remote_rpc_address) {
        return initWithMode({ ...mode, mode: 'normal' })
      }
      url = mode.remote_rpc_address
      break
    case 'service': {
      if (!mode.config_dir || !mode.file_log_dir || !mode.file_log_level || !mode.rpc_portal) {
        return initWithMode({ ...mode, mode: 'normal' })
      }
      let serviceStatus = await getServiceStatus()
      const coreVersion = await getEasytierVersion()
      if (serviceStatus === "NotInstalled" || modeConfigChanged(mode) || mode.installed_core_version !== coreVersion) {
        mode.config_server_url = mode.config_server_url || undefined
        await initService({
          config_dir: mode.config_dir,
          file_log_dir: mode.file_log_dir,
          file_log_level: mode.file_log_level,
          rpc_portal: mode.rpc_portal,
          config_server: mode.config_server_url,
        })
        mode.installed_core_version = coreVersion
        serviceStatus = await getServiceStatus()
      }
      if (serviceStatus === "Stopped") await setServiceStatus(true)
      url = "tcp://" + mode.rpc_portal.replace("0.0.0.0", "127.0.0.1")
      retrys = 5
      break
    }
    case 'normal':
      url = mode.rpc_portal
      break
  }
  for (let i = 0; i < retrys; i++) {
    try {
      await connectRpcClient(mode.mode === 'normal', url)
      break
    } catch (e) {
      if (i === retrys - 1) throw e
      await new Promise(resolve => setTimeout(resolve, 1000))
    }
  }
  await sendConfigs(running_inst_ids.map(Utils.UuidToStr))
  if (mode.mode === 'normal') {
    mode.config_server_url = mode.config_server_url || undefined
    initWebClient(mode.config_server_url)
  }
  currentMode.value = mode
  saveMode(mode)
  clientRunning.value = await isClientRunning().catch(() => false)
  addLog(clientRunning.value ? 'EasyTier prêt' : 'Backend non disponible (mode navigateur)')
}

onMounted(async () => {
  const cleanupFns: Array<() => void> = []
  if (type() === 'android') {
    try { await initMobileVpnService() } catch (e: any) { console.error(e) }
  }
  try { cleanupFns.push(await listenGlobalEvents()) } catch { /* preview web */ }
  currentMode.value = loadMode()
  if (!localStorage.getItem('lang')) {
    localStorage.setItem('lang', 'fr')
    uiLang.value = 'fr'
  }
  await setLanguage(uiLang.value)
  try {
    await initWithMode(currentMode.value)
  } catch (e) {
    clientRunning.value = false
    addLog('Démarrage sans backend Tauri — UI seule')
  }
  hostShareCode.value = ''
    hostStatus.value = s.value.noParty
  addLog('FangameLauncher démarré')
  onUnmounted(() => cleanupFns.forEach(fn => fn()))
})

useTray(true)
let toast = useToast()
const remoteClient = computed(() => new GUIRemoteClient())
const instanceId = ref<string | undefined>(undefined)
const clientRunning = ref(false)

watch(instanceId, (newVal) => { if (newVal) saveLastNetworkInstanceId(newVal) })

watch(clientRunning, async (newVal, oldVal) => {
  if (!newVal && oldVal) {
    if (manualDisconnect.value) { manualDisconnect.value = false; return }
    try { await reconnectClient() } catch { /* ignore in web */ }
  } else if (newVal && !oldVal) {
    const last = loadLastNetworkInstanceId()
    if (last) instanceId.value = last
  }
})

onMounted(async () => {
  clientRunning.value = await isClientRunning().catch(() => false)
  const timer = setInterval(async () => {
    try { clientRunning.value = await isClientRunning() } catch { clientRunning.value = false }
  }, 1000)
  onUnmounted(() => clearInterval(timer))
})

async function reconnectClient() {
  editingMode.value = JSON.parse(JSON.stringify(loadMode()))
  await onModeSave()
}

onMounted(async () => {
  window.setTimeout(async () => {
    try {
      await setTrayMenu([
        await MenuItemShow(t('tray.show')),
        await MenuItemExit(t('tray.exit')),
      ])
    } catch { /* web preview */ }
  }, 1000)
})

let current_log_level = 'off'
const log_menu = ref()
async function getLogDirPath(): Promise<string> {
  return await invoke<string>('get_log_dir_path')
}
const log_menu_items_popup: Ref<MenuItem[]> = ref([
  ...['off', 'warn', 'info', 'debug', 'trace'].map(level => ({
    label: () => t(`logging_level_${level}`) + (current_log_level === level ? ' ✓' : ''),
    command: async () => { current_log_level = level; await setLoggingLevel(level) },
  })),
  { separator: true },
  {
    label: () => t('logging_open_dir'),
    icon: 'pi pi-folder-open',
    command: async () => { await open(await getLogDirPath()) },
    visible: () => type() !== 'android',
  },
  {
    label: () => t('logging_copy_dir'),
    icon: 'pi pi-tablet',
    command: async () => { await writeText(await getLogDirPath()) },
  },
])
function toggle_log_menu(event: any) { log_menu.value.toggle(event) }
function getLabel(item: MenuItem) { return typeof item.label === 'function' ? item.label() : item.label }

const setting_menu_items: Ref<MenuItem[]> = ref([
  {
    label: () => t('mode.switch_mode') + ': ' + t('mode.' + currentMode.value.mode),
    icon: 'pi pi-sync',
    command: openModeDialog,
    visible: () => type() !== 'android',
  },
  {
    label: () => `${t('config-server.title')}${t('config-server.' + configServerConnectionStatus.value)}`,
    icon: 'pi pi-globe',
    command: openConfigServerDialog,
    visible: () => ["normal", "service"].includes(currentMode.value.mode),
  },
  { key: 'logging_menu', label: () => t('logging'), icon: 'pi pi-file', items: [] },
  { label: () => t('about.title'), icon: 'pi pi-at', command: async () => { aboutVisible.value = true } },
  { label: () => t('exit'), icon: 'pi pi-power-off', command: async () => { await exit(1) } },
])

async function connectRpcClient(isNormalMode: boolean, url?: string) {
  await initRpcConnection(isNormalMode, url)
}
async function openConfigServerDialog() {
  editingMode.value = JSON.parse(JSON.stringify(loadMode()))
  configServerDialogVisible.value = true
}
async function onConfigServerSave() {
  if (JSON.stringify(currentMode.value) === JSON.stringify(editingMode.value)) {
    configServerDialogVisible.value = false
    return
  }
  await onModeSave()
  configServerDialogVisible.value = false
}
onMounted(() => {
  const timer = setInterval(async () => {
    if (currentMode.value.mode !== 'normal') return
    if (!currentMode.value.config_server_url) return
    try { configServerConnected.value = await isWebClientConnected() } catch { /* */ }
  }, 1000)
  onUnmounted(() => clearInterval(timer))
})
const configServerConnectionStatus = computed(() => {
  if (currentMode.value.mode !== 'normal') return 'unknown'
  if (!currentMode.value.config_server_url) return 'disconnected'
  return configServerConnected.value ? 'connected' : 'connecting'
})
</script>

<template>
  <div id="root" class="fgl-root flex flex-col">
    <Dialog v-model:visible="aboutVisible" modal :header="t('about.title')" :style="{ width: '70%' }"><About /></Dialog>
    <Dialog v-model:visible="modeDialogVisible" modal :header="t('mode.switch_mode')" :style="{ width: '50vw' }">
      <ModeSwitcher v-model="editingMode" @uninstall-service="onUninstallService" @stop-service="onStopService" />
      <template #footer>
        <Button :label="t('web.common.cancel')" icon="pi pi-times" @click="modeDialogVisible = false" text />
        <Button :label="t('web.common.save')" icon="pi pi-save" @click="onModeSave" autofocus :loading="isModeSaving" />
      </template>
    </Dialog>
    <Dialog v-model:visible="configServerDialogVisible" modal :header="t('config-server.title')" :style="{ width: '50vw' }">
      <div class="flex flex-col gap-3">
        <label>{{ t('config-server.address') }}</label>
        <InputText v-model="(editingMode as WebClientConfig).config_server_url" />
      </div>
      <template #footer>
        <Button :label="t('web.common.cancel')" @click="configServerDialogVisible = false" text />
        <Button :label="t('web.common.save')" @click="onConfigServerSave" autofocus />
      </template>
    </Dialog>
    <Menu ref="log_menu" :model="log_menu_items_popup" :popup="true" />

    <!-- BANDEAU DEMO -->
    <div v-if="!clientRunning" class="fgl-banner-demo">
      MODE DEMO (preview web) — UI complete cliquable, reseau EasyTier non actif. Il manque le backend Tauri (disque / link.exe).
    </div>
    <!-- HEADER -->
    <header class="fgl-header">
      <div class="fgl-title">{{ s.title }}</div>
      <div class="fgl-lang">
        <span>{{ s.language }}</span>
        <select class="fgl-select" :value="uiLang" @change="setLanguage(($event.target as HTMLSelectElement).value)">
          <option value="fr">Français</option>
          <option value="en">English</option>
        </select>
      </div>
    </header>

    <!-- PSEUDO -->
    <div class="fgl-block">
      <label class="fgl-label">{{ s.pseudo }}</label>
      <input class="fgl-input" v-model="pseudo" type="text" maxlength="32" placeholder="Pseudo..." />
    </div>

    <!-- TABS -->
    <div class="fgl-tabs">
      <button type="button" class="fgl-tab" :class="{ active: activeTab === 'create' }" @click="activeTab = 'create'">{{ s.create }}</button>
      <button type="button" class="fgl-tab" :class="{ active: activeTab === 'join' }" @click="activeTab = 'join'">{{ s.join }}</button>
    </div>

    <!-- CREATE -->
    <div v-show="activeTab === 'create'" class="fgl-panel">
      <div class="fgl-row">
        <div class="fgl-field">
          <label class="fgl-label">{{ s.netName }}</label>
          <input class="fgl-input" v-model="hostNetworkName" type="text" />
        </div>
        <div class="fgl-field">
          <label class="fgl-label">{{ s.netSecret }}</label>
          <input class="fgl-input" v-model="hostNetworkSecret" type="password" />
        </div>
        <div class="fgl-field fgl-field-btns">
          <label class="fgl-label">&nbsp;</label>
          <div class="fgl-btns">
            <button type="button" class="fgl-btn green" :disabled="isBusy" @click="startHost">{{ s.startHost }}</button>
            <button type="button" class="fgl-btn red" :disabled="isBusy" @click="stopHost">{{ s.stopHost }}</button>
          </div>
        </div>
      </div>
      <div class="fgl-status-line">
        <span class="fgl-status">{{ hostStatus }}</span>
        <template v-if="hostShareCode">
          <span class="fgl-share-label">Code / peer a donner :</span>
          <input class="fgl-input fgl-share" :value="hostShareCode" readonly @focus="($event.target as HTMLInputElement).select()" />
          <button type="button" class="fgl-btn" @click="copyShareCode">Copier</button>
        </template>
      </div>
    </div>

    <!-- JOIN -->
    <div v-show="activeTab === 'join'" class="fgl-panel">
      <div class="fgl-row">
        <div class="fgl-field">
          <label class="fgl-label">{{ s.netName }}</label>
          <input class="fgl-input" v-model="joinNetworkName" type="text" />
        </div>
        <div class="fgl-field">
          <label class="fgl-label">{{ s.netSecret }}</label>
          <input class="fgl-input" v-model="joinNetworkSecret" type="password" />
        </div>
        <div class="fgl-field fgl-field-peer">
          <label class="fgl-label">{{ s.peerUrl }}</label>
          <input class="fgl-input" v-model="joinPeerUrl" type="text" :placeholder="s.peerPh" />
        </div>
        <div class="fgl-field fgl-field-btns">
          <label class="fgl-label">&nbsp;</label>
          <div class="fgl-btns">
            <button type="button" class="fgl-btn blue" :disabled="isBusy" @click="startJoin">{{ s.doJoin }}</button>
          </div>
        </div>
      </div>
      <div class="fgl-status">{{ joinStatus }}</div>
    </div>

    <!-- Advanced EasyTier (toute l'UI d'origine) -->
    <div class="fgl-adv">
      <button type="button" class="fgl-adv-toggle" @click="showAdvanced = !showAdvanced">
        {{ showAdvanced ? '▼' : '▶' }} {{ s.advanced }}
        <span v-if="!clientRunning" class="fgl-badge">backend off</span>
      </button>
      <div v-show="showAdvanced" class="fgl-adv-body">
        <RemoteManagement
          v-if="clientRunning"
          :api="remoteClient"
          :pause-auto-refresh="isModeSaving"
          v-model:instance-id="instanceId"
        />
        <div v-else class="fgl-adv-off">
          <p><strong>Backend OFF</strong> — preview navigateur : l UI complete EasyTier (RemoteManagement) s affiche seulement avec Tauri + link.exe.</p>
          <p>Ce que tu auras en mode natif dans cette zone :</p>
          <ul>
            <li>Liste des reseaux / instances</li>
            <li>Creer / editer / lancer / arreter un reseau</li>
            <li>Import / export config TOML</li>
            <li>Statut, peers, routes, erreurs</li>
            <li>Tous les reglage avances EasyTier (Config.vue)</li>
          </ul>
          <p>Formulaire simplifie actuel (onglets Creer / Rejoindre) = raccourci joueur vers la meme logique run_network.</p>
        </div>
      </div>
    </div>

    <!-- LOGS + CHAT FUSIONNÉS (comme main.py) -->
    <div class="fgl-bottom">
      <div class="fgl-label">{{ s.logsChat }}</div>
      <div id="fgl-logbox" class="fgl-logbox">
        <div v-for="(line, i) in logLines" :key="i" class="fgl-logline">{{ line }}</div>
      </div>
      <div class="fgl-chatrow">
        <input class="fgl-input flex1" v-model="chatInput" @keyup.enter="sendChat" type="text" placeholder="..." />
        <button type="button" class="fgl-btn" @click="sendChat">{{ s.send }}</button>
      </div>
    </div>

    <Menubar :model="setting_menu_items" breakpoint="795px" class="fgl-menubar">
      <template #item="{ item, props }">
        <a v-if="item.key === 'logging_menu'" v-bind="props.action" @click="toggle_log_menu">
          <span :class="item.icon" /><span class="p-menubar-item-label">{{ getLabel(item) }}</span>
        </a>
        <a v-else v-bind="props.action">
          <span :class="item.icon" /><span class="p-menubar-item-label">{{ getLabel(item) }}</span>
        </a>
      </template>
    </Menubar>
  </div>
</template>

<style scoped lang="postcss">
.fgl-root {
  height: 100vh;
  width: 100vw;
  background: #121212;
  color: #fff;
  overflow: hidden;
  font-size: 13px;
  display: flex;
  flex-direction: column;
}
.fgl-banner-demo {
  background: #5d4037;
  color: #ffe0b2;
  font-size: 12px;
  padding: 6px 12px;
  text-align: center;
  flex-shrink: 0;
}
.fgl-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: #1e1e1e;
  border-bottom: 1px solid #333;
  flex-shrink: 0;
}
.fgl-title { font-size: 1.15rem; font-weight: 700; color: #4fc3f7; }
.fgl-lang { display: flex; align-items: center; gap: 6px; color: #ddd; font-size: 12px; }
.fgl-block {
  padding: 6px 12px;
  background: #1a1a1a;
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
.fgl-block .fgl-input { max-width: 200px; }
.fgl-label { font-size: 12px; color: #eee; font-weight: 600; white-space: nowrap; }
.fgl-input {
  background: #252525;
  color: #fff;
  border: 1px solid #444;
  border-radius: 3px;
  padding: 5px 8px;
  font-size: 13px;
  height: 28px;
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
}
.fgl-input:focus { outline: 1px solid #4fc3f7; }
.fgl-select {
  background: #252525; color: #fff; border: 1px solid #444;
  border-radius: 3px; padding: 4px 8px; font-size: 12px; height: 28px;
}
.fgl-tabs { display: flex; padding: 0 12px; background: #1a1a1a; flex-shrink: 0; }
.fgl-tab {
  flex: 1; padding: 8px; font-size: 13px; font-weight: 700;
  background: #252525; color: #bbb; border: 1px solid #333; cursor: pointer;
}
.fgl-tab.active { color: #fff; background: #2e7d32; border-color: #43a047; }
.fgl-tab:last-child.active { background: #1565c0; border-color: #1e88e5; }
.fgl-panel {
  padding: 8px 12px;
  background: #1e1e1e;
  flex-shrink: 0;
}
.fgl-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: flex-end;
}
.fgl-field { flex: 1 1 120px; min-width: 100px; max-width: 200px; display: flex; flex-direction: column; gap: 3px; }
.fgl-field-peer { flex: 1 1 180px; max-width: 260px; }
.fgl-field-btns { flex: 0 0 auto; max-width: none; }
.fgl-btns { display: flex; gap: 6px; }
.fgl-btn {
  background: #333; color: #fff; border: 1px solid #555;
  padding: 5px 12px; border-radius: 3px; font-size: 12px; font-weight: 600;
  cursor: pointer; height: 28px; white-space: nowrap;
}
.fgl-btn:hover { filter: brightness(1.12); }
.fgl-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.fgl-btn.green { background: #43a047; border-color: #66bb6a; }
.fgl-btn.blue { background: #1e88e5; border-color: #42a5f5; }
.fgl-btn.red { background: #e53935; border-color: #ef5350; }
.fgl-status { color: #4fc3f7; font-size: 12px; }
.fgl-status-line {
  display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-top: 6px;
}
.fgl-share-label { font-size: 12px; color: #ccc; }
.fgl-share { max-width: 220px; flex: 1 1 160px; }
.fgl-adv { border-top: 1px solid #333; background: #181818; flex-shrink: 0; }
.fgl-adv-toggle {
  width: 100%; text-align: left; padding: 6px 12px; background: transparent;
  border: none; color: #aaa; font-size: 12px; cursor: pointer;
}
.fgl-badge {
  margin-left: 6px; font-size: 11px; color: #e53935;
  border: 1px solid #e53935; padding: 0 5px; border-radius: 3px;
}
.fgl-adv-body { padding: 6px; max-height: 22vh; overflow-y: auto; }
.fgl-adv-off { color: #888; padding: 8px; font-size: 12px; }
.fgl-bottom {
  flex: 1; min-height: 80px; display: flex; flex-direction: column;
  padding: 6px 12px 8px; background: #121212; border-top: 1px solid #333;
  min-height: 0;
}
.fgl-logbox {
  flex: 1; min-height: 60px; overflow-y: auto; background: #0a0a0a;
  border: 1px solid #333; padding: 6px 8px; font-family: Consolas, monospace;
  font-size: 12px; color: #e0e0e0; margin: 4px 0;
}
.fgl-logline { white-space: pre-wrap; word-break: break-all; }
.fgl-chatrow { display: flex; gap: 6px; flex-shrink: 0; }
.fgl-chatrow .flex1, .fgl-chatrow .fgl-input { flex: 1; }
.fgl-menubar { background: #1e1e1e !important; border-top: 1px solid #333; flex-shrink: 0; }
</style>

<style>
body {
  height: 100vh;
  width: 100vw;
  margin: 0;
  padding: 0;
  overflow: hidden;
  background: #121212 !important;
  color: #fff;
}
</style>









