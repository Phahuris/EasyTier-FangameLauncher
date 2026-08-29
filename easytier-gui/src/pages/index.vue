<script setup lang="ts">
import { type } from '@tauri-apps/plugin-os'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
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
const logLines = ref<{ ts: string, text: string, kind: string }[]>([])
const isBusy = ref(false)
const showAdvanced = ref(false)

const peerList = ref<string[]>([])
const peerIps = ref<string[]>([])
const knownPeerNames = ref<string[]>([])
const isNetworkActive = ref(false)
const useDhcp = ref(true)
const publicNodeUrl = ref(localStorage.getItem('fgl_public_node') || 'tcp://easytier-us.slarker.me:11010')
const publicNodeCandidates = [
  'tcp://easytier-us.slarker.me:11010',
  'tcp://us01.225284.xyz:11010',
]
watch(publicNodeUrl, (v) => localStorage.setItem('fgl_public_node', v))


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
const fangamePath = ref('')
const fangameTitle = ref('')
const fangameExeSize = ref(0)
const hostFangameFp = ref<{ title: string, exe_size: number } | null>(null)
const fangameInfo = ref<any>(null)
const joinCode = ref('')

watch(pseudo, (v) => localStorage.setItem('fgl_pseudo', v))
watch(hostNetworkName, (v) => {
  localStorage.setItem('fgl_net_name', v)
  joinNetworkName.value = v
})
watch(hostNetworkSecret, (v) => {
  localStorage.setItem('fgl_net_secret', v)
  joinNetworkSecret.value = v
})

function addLog(msg: string, kind: string = 'info') {
  const ts = new Date().toLocaleTimeString()
  logLines.value.push({ ts, text: msg, kind })
  if (logLines.value.length > 400) logLines.value.shift()
  nextTick(() => {
    const el = document.getElementById('fgl-logbox')
    if (el) el.scrollTop = el.scrollHeight
  })
}

function logShareCode(_code: string) {
  // Code affiche dans le bandeau host (bouton Copier) — pas dans les logs
}

function copyShareCode() {
  const code = hostShareCode.value
  if (!code) return

  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(code)
      .then(() => addLog('Code copie'))
      .catch(() => addLog('Copie impossible'))
  } else {
    addLog('Code: ' + code)
  }
}

async function copyLogsChat() {
  const content = logLines.value.map(l => '[' + l.ts + '] ' + l.text).join('\n')

  if (!content) {
    addLog('Logs / Chat vide')
    return
  }

  try {
    await writeText(content)
    addLog('Logs / Chat copiés')
  }
  catch (e) {
    console.error('[LOGS] Copie impossible:', e)

    try {
      await navigator.clipboard.writeText(content)
      addLog('Logs / Chat copiés')
    }
    catch (e2) {
      console.error('[LOGS] Clipboard navigateur impossible:', e2)
      addLog('Copie des Logs / Chat impossible')
    }
  }
}

const uiLang = ref(localStorage.getItem('lang') || 'fr')
const uiStrings: Record<string, Record<string, string>> = {
  fr: {
    title: 'FangameLauncher',
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
    publicNode: 'Noeud public',
    fangame: 'Fangame (bientot)',
    fangamePh: 'Selection du fangame — bientot disponible',
    partyCode: 'Code de partie',
    partyCodePh: 'colle le code : nom|mdp|noeud',
    connected: 'Connecte au reseau',
    playerJoined: ' a rejoint la partie',
    hostStopped: 'Partie arretee.',
    searchingNode: 'Recherche d un noeud public...',
    nodeOk: 'Noeud public OK',
    nodeFail: 'Noeud indisponible',
    nodeFallback: 'Noeud de secours',
  },
  en: {
    title: 'FangameLauncher',
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
    publicNode: 'Public node',
    fangame: 'Fangame (soon)',
    fangamePh: 'Fangame selection — coming soon',
    partyCode: 'Party code',
    partyCodePh: 'paste code: name|password|node',
    connected: 'Connected to network',
    playerJoined: ' joined the party',
    hostStopped: 'Party stopped.',
    searchingNode: 'Looking for a public node...',
    nodeOk: 'Public node OK',
    nodeFail: 'Node unavailable',
    nodeFallback: 'Fallback node',
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
    dhcp: useDhcp.value,
    virtual_ipv4: '',
    network_length: 24,
    hostname: pseudo.value.trim(),
    network_name: hostNetworkName.value.trim() || 'fangame',
    network_secret: hostNetworkSecret.value,
    networking_method: 1,
    public_server_url: '',
    peer_urls: [publicNodeUrl.value.trim() || publicNodeCandidates[0]],
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
    dhcp: useDhcp.value,
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




function normalizeInstIds(listed: any): string[] {
  const raw: any[] = Array.isArray(listed)
    ? listed
    : (Array.isArray(listed?.running_inst_ids) ? listed.running_inst_ids : [])
  return raw.map((id: any) => {
    if (typeof id === 'string') return id
    try { return Utils.UuidToStr(id) } catch { return String(id ?? '') }
  }).filter((s: string) => !!s)
}

function u32ToIpv4(n: number): string {
  const x = (Number(n) >>> 0)
  return ((x >>> 24) & 255) + '.' + ((x >>> 16) & 255) + '.' + ((x >>> 8) & 255) + '.' + (x & 255)
}

function extractIp(v: any): string {
  if (v === null || v === undefined || v === '') return ''
  if (typeof v === 'string') return v.split('/')[0].trim()
  if (typeof v === 'number') return u32ToIpv4(v)
  if (typeof v === 'object') {
    if (v.address !== undefined) {
      if (typeof v.address === 'number') return u32ToIpv4(v.address)
      if (v.address && typeof v.address.addr === 'number') return u32ToIpv4(v.address.addr)
      if (typeof v.address === 'string') return String(v.address).split('/')[0].trim()
    }
    if (typeof v.addr === 'number') return u32ToIpv4(v.addr)
    for (const k of ['ip', 'ipv4', 'virtual_ipv4', 'ipv4_addr']) {
      if (v[k] !== undefined) {
        const r = extractIp(v[k])
        if (r) return r
      }
    }
  }
  return ''
}

function extractHostname(p: any): string {
  if (!p || typeof p !== 'object') return ''
  return String(p.hostname || p.host_name || p.name || p?.route?.hostname || p?.peer?.hostname || '').trim()
}

function isPublicServerName(name: string): boolean {
  const n = (name || '').toLowerCase()
  return !name || n.includes('publicserver') || n === 'server' || n.startsWith('public')
}

function collectPeersFromNetwork(network: any): { names: string[], ips: string[] } {
  const names = new Set<string>()
  const ips = new Set<string>()
  if (!network || typeof network !== 'object') return { names: [], ips: [] }
  const myIp = extractIp(network?.my_node_info?.virtual_ipv4 || network?.my_node_info?.virtual_ip)
  const myHn = String(network?.my_node_info?.hostname || '').trim()
  if (myHn && !isPublicServerName(myHn)) names.add(myHn)

  const pairs = Array.isArray(network.peer_route_pairs) ? network.peer_route_pairs : []
  for (const pair of pairs) {
    const route = pair?.route || pair
    if (!route || route?.feature_flag?.is_public_server) continue
    const hn = String(route.hostname || '').trim()
    const ip = extractIp(route.ipv4_addr || route.virtual_ipv4)
    if (hn && !isPublicServerName(hn)) names.add(hn)
    if (ip && ip !== myIp) ips.add(ip)
  }

  const peers = Array.isArray(network.peers) ? network.peers : []
  for (const p of peers) {
    if (!p || typeof p !== 'object') continue
    const hn = extractHostname(p)
    const ip = extractIp(p.virtual_ipv4 || p.ipv4_addr || p.ipv4)
    if (hn && !isPublicServerName(hn)) names.add(hn)
    if (ip && ip !== myIp) ips.add(ip)
  }
  return { names: [...names], ips: [...ips] }
}

async function sendChat() {
  const msg = chatInput.value.trim()
  if (!msg) return
  const name = pseudo.value.trim() || 'Anonyme'
  chatInput.value = ''
  addLog(name + ' : ' + msg, 'chat')
  if (!clientRunning.value || !isNetworkActive.value) return
  try {
    await invoke('chat_start')
    await refreshPeers()
    let peers = [...peerIps.value]
    if (peers.length === 0) {
      const result = await invoke<any>('list_network_instance_ids')
      const ids = normalizeInstIds(result)
      for (const id of ids) {
        try {
          const info = await invoke<any>('collect_network_info', { inst_id: id })
          const network = info?.info?.map?.[id] || info?.map?.[id] || info
          peers.push(...collectPeersFromNetwork(network).ips)
        } catch { /* ignore */ }
      }
      peers = [...new Set(peers)]
    }
    if (peers.length === 0) {
      return
    }
    await invoke('chat_send', { pseudo: name, text: msg, peers })
  } catch (e) {
    addLog('[Chat] Envoi reseau: ' + String(e), 'warn')
  }
}

async function refreshPeers() {
  if (!clientRunning.value) {
    peerList.value = pseudo.value.trim() ? [pseudo.value.trim()] : []
    peerIps.value = []
    return
  }
  try {
    const result = await invoke<any>('list_network_instance_ids')
    const ids = normalizeInstIds(result)
    const allNames = new Set<string>()
    const allIps = new Set<string>()
    if (pseudo.value.trim()) allNames.add(pseudo.value.trim())

    for (const id of ids) {
      try {
        let network: any = null
        try {
          if (remoteClient?.value?.get_network_info) {
            network = await remoteClient.value.get_network_info(id)
          }
        } catch { /* ignore */ }
        if (!network) {
          const info = await invoke<any>('collect_network_info', { inst_id: id })
          network = info?.info?.map?.[id] || info?.map?.[id] || info
        }
        const { names, ips } = collectPeersFromNetwork(network)
        names.forEach(n => allNames.add(n))
        ips.forEach(ip => allIps.add(ip))
      } catch { /* ignore */ }
    }

    const prev = new Set(knownPeerNames.value.map(x => x.toLowerCase()))
    const me = (pseudo.value.trim() || '').toLowerCase()
    for (const n of allNames) {
      const key = n.toLowerCase()
      if (key && key !== me && !prev.has(key) && knownPeerNames.value.length > 0) {
        addLog(n + s.value.playerJoined, 'join')
      }
    }

    const nameList = [...allNames]
    const ipList = [...allIps]
    const changed =
      nameList.join('|') !== peerList.value.join('|') ||
      ipList.join('|') !== peerIps.value.join('|')

    knownPeerNames.value = nameList
    peerList.value = nameList
    peerIps.value = ipList

    if (changed) {
      addLog(
        'Scan peers: ' + nameList.length + ' nom(s) [' + nameList.join(', ') + '] / ' +
        ipList.length + ' IP [' + ipList.join(', ') + ']',
        ipList.length > 0 ? 'ok' : 'warn'
      )
    }
  } catch {
    if (pseudo.value.trim()) peerList.value = [pseudo.value.trim()]
  }
}
onMounted(() => {
  const peerTimer = setInterval(() => {
    if (isNetworkActive.value && clientRunning.value) {
      refreshPeers().catch(() => {})
    }
  }, 3000)
  onUnmounted(() => clearInterval(peerTimer))
})

async function browseFangame() {
  try {
    const info = await invoke<any>('pick_fangame_and_detect')
    if (info.cancelled) {
      addLog(uiLang.value === 'fr' ? 'Selection annulee' : 'Selection cancelled', 'info')
      return
    }
    fangameInfo.value = info
    if (info.root) fangamePath.value = info.root
    else if (info.game_exe) fangamePath.value = info.game_exe
    fangameTitle.value = info.game_title || ''
    fangameExeSize.value = Number(info.game_exe_size || 0)
    addLog(info.message, info.ok ? 'ok' : 'warn')
    if (info.mode && info.mode !== 'unknown') addLog('Mode: ' + info.mode, 'ok')
    if (info.scripts_rxdata) addLog('Scripts: ' + info.scripts_rxdata, 'ok')
    if (info.game_rgssad) addLog('RGSSAD: ' + info.game_rgssad, 'ok')
  } catch (e) {
    addLog('Fangame: ' + String(e), 'warn')
  }
}

async function ensureFangameReady(): Promise<boolean> {
  const path = (fangamePath.value || '').trim()
  if (!path) {
    addLog(uiLang.value === 'fr' ? 'Choisis un fangame (bouton ...)' : 'Select a fangame (... button)', 'warn')
    return false
  }
  try {
    // 1 extract Scripts si rgssad  2 check plugin  3 inject seulement si absent
    const msg = await invoke<string>('prepare_and_patch_fangame', { gamePath: path })
    addLog(String(msg), 'ok')
    return true
  } catch (e) {
    addLog('Patch: ' + String(e), 'warn')
    return false
  }
}

async function launchSelectedFangame() {
  const path = (fangamePath.value || '').trim()
  if (!path) return
  try {
    await invoke('launch_fangame', { path })
    addLog(uiLang.value === 'fr' ? 'Jeu lance' : 'Game launched', 'ok')
  } catch (e) {
    addLog('Launch: ' + String(e), 'warn')
  }
}
async function pickPublicNode(): Promise<string> {
  const list = [
    publicNodeUrl.value.trim(),
    ...publicNodeCandidates,
  ].filter((v, i, a) => !!v && a.indexOf(v) === i)

  addLog(s.value.searchingNode)
  for (const url of list) {
    const m = url.match(/^(?:tcp|udp):\/\/([^:\/\s]+):(\d+)/i)
    if (!m) continue
    const host = m[1]
    try {
      const ctrl = new AbortController()
      const timer = setTimeout(() => ctrl.abort(), 3500)
      const res = await fetch(
        'https://dns.google/resolve?name=' + encodeURIComponent(host) + '&type=A',
        { signal: ctrl.signal }
      )
      clearTimeout(timer)
      const data = await res.json()
      if (Array.isArray(data?.Answer) && data.Answer.length > 0) {
        publicNodeUrl.value = url
        addLog(s.value.nodeOk + ' : ' + url)
        return url
      }
      addLog(s.value.nodeFail + ' : ' + url)
    } catch {
      addLog(s.value.nodeFail + ' : ' + url)
    }
  }
  const fallback = publicNodeCandidates[0]
  publicNodeUrl.value = fallback
  addLog(s.value.nodeFallback + ' : ' + fallback)
  return fallback
}
async function startHost() {
  if (!requirePseudo()) return
  if (!hostNetworkName.value.trim()) { addLog(s.value.needName); return }

  isBusy.value = true
  try {
    if (!(await ensureFangameReady())) { isBusy.value = false; return }
    try {
      const fp = await invoke<any>('get_fangame_fingerprint', { path: fangamePath.value })
      hostFangameFp.value = fp
      localStorage.setItem('fgl_host_fp', JSON.stringify(fp))
    } catch (e) { addLog('Fangame fingerprint: ' + String(e), 'warn'); isBusy.value = false; return }
    const node = await pickPublicNode()
    publicNodeUrl.value = node

    if (!clientRunning.value) {
      hostShareCode.value = [hostNetworkName.value.trim(), hostNetworkSecret.value, node].join('|')
      hostStatus.value = 'DEMO'
      isNetworkActive.value = true
      addLog('MODE DEMO - code: ' + hostShareCode.value)
      return
    }

    const cfg = buildHostConfig()
    cfg.peer_urls = [node]
    await remoteClient.value.run_network(cfg, true)
    instanceId.value = cfg.instance_id
    hostStatus.value = s.value.hostRunning
    isNetworkActive.value = true
    addLog(s.value.connected, 'ok')
    refreshPeers()
    hostShareCode.value = [hostNetworkName.value.trim(), hostNetworkSecret.value, node].join('|')
    addLog(s.value.hostOk)
    logShareCode(hostShareCode.value)
    await launchSelectedFangame() /* no log */
  } catch (e: unknown) {
    addLog('Erreur host: ' + String(e))
    hostStatus.value = 'Erreur'
    isNetworkActive.value = false
  } finally {
    isBusy.value = false
  }
}

async function stopHost() {
  isBusy.value = true
  try {
    if (instanceId.value && clientRunning.value) {
      try {
        await remoteClient.value.update_network_instance_state(instanceId.value, true)
      } catch (e) {
        console.warn('stop network', e)
      }
    }
  } finally {
    // IMPORTANT : reset UI meme si le backend echoue
    instanceId.value = undefined
    hostShareCode.value = ''
    hostStatus.value = 'Aucune partie'
    joinStatus.value = ''
    isNetworkActive.value = false
    peerList.value = []
    isBusy.value = false
    addLog(s.value.hostStopped)
  }
}
async function startJoin() {
  if (!requirePseudo()) return
  // Accepte le code complet dans joinCode OU joinPeerUrl : nom|mdp|noeud
  const raw = (joinCode.value || joinPeerUrl.value || '').trim()
  if (raw.includes('|')) {
    const parts = raw.split('|')
    if (parts.length >= 2) {
      joinNetworkName.value = (parts[0] || '').trim() || joinNetworkName.value
      joinNetworkSecret.value = (parts[1] || '').trim()
      if (parts[2]) {
        const node = parts.slice(2).join('|').trim()
        joinPeerUrl.value = node
        publicNodeUrl.value = node
      }
      joinCode.value = raw
    }
  } else if (raw && (raw.startsWith('tcp://') || raw.startsWith('udp://'))) {
    joinPeerUrl.value = raw
  }

  if (!joinNetworkName.value.trim()) { addLog(s.value.needName); return }
  if (!joinPeerUrl.value.trim()) { addLog(s.value.needPeer); return }

  isBusy.value = true
  try {
    if (!(await ensureFangameReady())) { isBusy.value = false; return }
    try {
      const fp = await invoke<any>('get_fangame_fingerprint', { path: fangamePath.value })
      const hostFpRaw = localStorage.getItem('fgl_host_fp')
      // Si on a une empreinte host locale (meme PC test) ou future partage reseau
      if (hostFpRaw) {
        const hostFp = JSON.parse(hostFpRaw)
        if (hostFp.title && fp.title && hostFp.title !== fp.title) {
          addLog(uiLang.value === 'fr' ? 'Mauvais fangame (titre different)' : 'Wrong fangame (different title)', 'warn')
          isBusy.value = false; return
        }
        if (hostFp.exe_size && fp.exe_size && hostFp.exe_size !== fp.exe_size) {
          addLog(uiLang.value === 'fr' ? 'Mauvais fangame (version differente)' : 'Wrong fangame (different version)', 'warn')
          isBusy.value = false; return
        }
      }
    } catch (e) { addLog('Fangame check: ' + String(e), 'warn'); isBusy.value = false; return }
    if (!clientRunning.value) {
      joinStatus.value = 'DEMO ÔCö join simule (pas de backend)'
      addLog('MODE DEMO JOIN')
                              return
    }
    const cfg = buildJoinConfig()
    await remoteClient.value.run_network(cfg, true)
    instanceId.value = cfg.instance_id
    joinStatus.value = 'Connecte...'
    isNetworkActive.value = true
    addLog(s.value.connected, 'ok')
    refreshPeers()
    addLog(s.value.joinOk)
    await launchSelectedFangame()
      } catch (e: unknown) {
    addLog('Erreur join: ' + String(e))
    joinStatus.value = 'Erreur'
  } finally {
    isBusy.value = false
  }
}

// ===== EasyTier mode / RPC (conserv├®) =====
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
    addLog('Mode sauvegard├®')
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

  try {
    const unlistenChat = await listen<any>('chat_message', (event) => {
      const pkt = event.payload

      console.log('[CHAT RX GUI] Message re├ºu:', pkt)

      if (!pkt) {
        return
      }

      if (pkt.type === 'chat' || pkt.kind === 'chat') {
        const sender = String(pkt.pseudo || 'Anonyme')
        const text = String(pkt.text || '')

        if (text) {
          addLog(sender + ' : ' + text, 'chat')
        }
      }
      else if (pkt.type === 'cmd' || pkt.kind === 'cmd') {
        addLog(
          `[CMD] ${pkt.pseudo || 'Anonyme'} ÔåÆ ${pkt.plugin || ''} / ${pkt.action || ''}`
        )
      }
    })

    cleanupFns.push(unlistenChat)
  }
  catch (e) {
    console.warn('[CHAT] ├ecoute chat indisponible:', e)
  }
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
    addLog('D├®marrage sans backend Tauri ÔCö UI seule')
  }
  hostShareCode.value = ''
    hostStatus.value = s.value.noParty
  addLog('FangameLauncher démarré')
  onUnmounted(() => cleanupFns.forEach(fn => fn()))
})

useTray(false)

// Fermer la fenetre = quitter l app (pas rester en tray)
onMounted(async () => {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    const win = getCurrentWindow()
    await win.onCloseRequested(async () => {
      try { await exit(0) } catch { /* ignore */ }
    })
  } catch { /* preview web */ }
})

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
    label: () => t(`logging_level_${level}`) + (current_log_level === level ? ' Ô£o' : ''),
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
  <div id="root" class="fgl-root">
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
      MODE DEMO - backend EasyTier non actif
    </div>

    <!-- ONGLET STYLE CHROME + LANGUE -->
    <div class="fgl-chrome">
      <div class="fgl-chrome-tabs">
        <button type="button" class="fgl-chrome-tab" :class="{ active: activeTab === 'create' }" @click="activeTab = 'create'">{{ s.create }}</button>
        <button type="button" class="fgl-chrome-tab" :class="{ active: activeTab === 'join' }" @click="activeTab = 'join'">{{ s.join }}</button>
      </div>
      <div class="fgl-chrome-right">
        <select class="fgl-select" :value="uiLang" @change="setLanguage(($event.target as HTMLSelectElement).value)">
          <option value="fr">Français</option>
          <option value="en">English</option>
        </select>
      </div>
    </div>

        <!-- CREATE -->
    <div v-show="activeTab === 'create'" class="fgl-panel">
      <div v-if="!isNetworkActive" class="fgl-card">
        <div class="fgl-grid fgl-grid-create">
          <div class="fgl-field">
            <label class="fgl-label">{{ s.pseudo }}</label>
            <input class="fgl-input" v-model="pseudo" type="text" maxlength="32" placeholder="Pseudo..." />
          </div>
          <div class="fgl-field">
            <label class="fgl-label">{{ s.netName }}</label>
            <input class="fgl-input" v-model="hostNetworkName" type="text" />
          </div>
          <div class="fgl-field">
            <label class="fgl-label">{{ s.netSecret }}</label>
            <input class="fgl-input" v-model="hostNetworkSecret" type="password" />
          </div>
          <div class="fgl-field">
            <label class="fgl-label">{{ s.publicNode || 'Public node' }}</label>
            <input class="fgl-input" v-model="publicNodeUrl" type="text" placeholder="tcp://easytier-us.slarker.me:11010" />
          </div>
          <div class="fgl-field fgl-field-half">
            <label class="fgl-label">{{ s.fangame || 'Fangame' }}</label>
            <div class="fgl-fangame-row">
              <input class="fgl-input" v-model="fangamePath" type="text" :placeholder="s.fangamePh || 'Game.exe'" />
              <button type="button" class="fgl-btn" @click="browseFangame" title="Browse">...</button>
            </div>
          </div>
          <div class="fgl-field fgl-field-half">
            <label class="fgl-label">&nbsp;</label>
            <div v-if="fangameTitle" class="fgl-fangame-title">{{ fangameTitle }}</div>
          </div>
        </div>
        <div class="fgl-actions">
          <button type="button" class="fgl-btn green" :disabled="isBusy" @click="startHost">{{ s.startHost }}</button>
        </div>
      </div>
      <div v-else class="fgl-card fgl-card-running">
        <div class="fgl-running-line"><span>Reseau</span><strong>{{ hostNetworkName }}</strong></div>
        <div class="fgl-running-line"><span>Noeud</span><strong>{{ publicNodeUrl }}</strong></div>
        <div class="fgl-running-line" v-if="hostShareCode">
          <span>Code</span>
          <input class="fgl-input fgl-share" :value="hostShareCode" readonly @focus="($event.target as HTMLInputElement).select()" />
          <button type="button" class="fgl-btn" @click="copyShareCode">Copier</button>
        </div>
        <div class="fgl-actions">
          <button type="button" class="fgl-btn red" :disabled="isBusy" @click="stopHost">{{ s.stopHost }}</button>
        </div>
        <div class="fgl-status">{{ hostStatus }}</div>
      </div>
    </div>

    <!-- JOIN -->
    <div v-show="activeTab === 'join'" class="fgl-panel">
      <div v-if="!isNetworkActive" class="fgl-card">
        <div class="fgl-grid fgl-grid-join">
          <div class="fgl-field fgl-field-pseudo">
            <label class="fgl-label">{{ s.pseudo }}</label>
            <input class="fgl-input" v-model="pseudo" type="text" maxlength="32" placeholder="Pseudo..." />
          </div>
          <div class="fgl-field fgl-field-code">
            <label class="fgl-label">{{ s.partyCode || 'Party code' }}</label>
            <input class="fgl-input" v-model="joinCode" type="text" :placeholder="s.partyCodePh || 'name|password|node'" />
          </div>
          <div class="fgl-field fgl-field-half">
            <label class="fgl-label">{{ s.fangame || 'Fangame' }}</label>
            <div class="fgl-fangame-row">
              <input class="fgl-input" v-model="fangamePath" type="text" :placeholder="s.fangamePh || 'Game.exe'" />
              <button type="button" class="fgl-btn" @click="browseFangame" title="Browse">...</button>
            </div>
          </div>
          <div class="fgl-field fgl-field-half">
            <label class="fgl-label">&nbsp;</label>
            <div v-if="fangameTitle" class="fgl-fangame-title">{{ fangameTitle }}</div>
          </div>
        </div>
        <div class="fgl-actions">
          <button type="button" class="fgl-btn blue" :disabled="isBusy" @click="startJoin">{{ s.doJoin }}</button>
        </div>
      </div>
      <div v-else class="fgl-card fgl-card-running">
        <div class="fgl-running-line"><span>Reseau</span><strong>{{ joinNetworkName }}</strong></div>
        <div class="fgl-running-line"><span>Noeud</span><strong>{{ joinPeerUrl || publicNodeUrl }}</strong></div>
        <div class="fgl-actions">
          <button type="button" class="fgl-btn red" :disabled="isBusy" @click="stopHost">{{ s.stopHost }}</button>
        </div>
        <div class="fgl-status">{{ joinStatus }}</div>
      </div>
    </div>

    <!-- AVANCE -->
    <div class="fgl-adv">
      <button type="button" class="fgl-adv-toggle" @click="showAdvanced = !showAdvanced">
        {{ showAdvanced ? '▼' : '▶' }} {{ s.advanced }} <span class="fgl-adv-note">(pas nécessaire — déjà configuré par défaut)</span>
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
          Backend OFF - options EasyTier completes avec Tauri.
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
    </div>

    <!-- JOUEURS + LOGS -->
    <div class="fgl-bottom">
      <div class="fgl-peers">
        <div class="fgl-label">Joueurs</div>
        <div class="fgl-peerbox">
          <div v-if="peerList.length === 0" class="fgl-peer-empty">-</div>
          <div v-for="(name, i) in peerList" :key="i" class="fgl-peer">{{ name }}</div>
        </div>
      </div>
      <div class="fgl-logs-wrap">
        <div class="fgl-label">{{ s.logsChat }}</div>
        <div id="fgl-logbox" class="fgl-logbox">
          <div v-for="(line, i) in logLines" :key="i" class="fgl-logline" :class="'fgl-log-' + (line.kind || 'info')"><span class="fgl-log-ts">[{{ line.ts }}]</span> {{ line.text }}</div>
        </div>
        <div class="fgl-chatrow">
          <input class="fgl-input flex1" v-model="chatInput" @keyup.enter="sendChat" type="text" placeholder="..." />
          <button type="button" class="fgl-btn" @click="sendChat">{{ s.send }}</button>          <button type="button" class="fgl-btn fgl-copy-logs" @click="copyLogsChat">Copier Logs / Chat</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped lang="postcss">
.fgl-root {
  height: 100vh;
  width: 100vw;
  background: #121212;
  color: #e8e8e8;
  overflow: hidden;
  font-size: 13px;
  display: flex;
  flex-direction: column;
  font-family: "Segoe UI", system-ui, sans-serif;
}
.fgl-banner-demo {
  background: #5d4037;
  color: #ffe0b2;
  font-size: 12px;
  padding: 6px 12px;
  text-align: center;
  flex-shrink: 0;
}
.fgl-chrome {
  display: flex;
  align-items: stretch;
  background: #1a1a1a;
  border-bottom: 1px solid #2a2a2a;
  flex-shrink: 0;
  min-height: 40px;
}
.fgl-chrome-tabs {
  display: flex;
  flex: 1;
  padding: 6px 6px 0 8px;
}
.fgl-chrome-tab {
  min-width: 140px;
  max-width: 220px;
  padding: 8px 18px;
  font-size: 13px;
  font-weight: 600;
  background: #252525;
  color: #9e9e9e;
  border: 1px solid #333;
  border-bottom: none;
  border-radius: 10px 10px 0 0;
  cursor: pointer;
  margin-right: 2px;
}
.fgl-chrome-tab:hover { color: #ddd; background: #2c2c2c; }
.fgl-chrome-tab.active {
  background: #121212;
  color: #fff;
  border-color: #3a3a3a;
}
.fgl-chrome-right {
  display: flex;
  align-items: center;
  padding: 0 12px;
}
.fgl-panel { padding: 12px 14px 8px; background: #121212; flex-shrink: 0; }
.fgl-card {
  background: #1c1c1c;
  border: 1px solid #2e2e2e;
  border-radius: 10px;
  padding: 14px;
}
.fgl-card-running { border-color: #2e7d32; }
.fgl-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 10px 12px;
}
.fgl-field { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.fgl-field-wide { grid-column: 1 / -1; }
.fgl-label { font-size: 12px; color: #b0b0b0; font-weight: 600; }
.fgl-input {
  background: #252525;
  color: #fff;
  border: 1px solid #3a3a3a;
  border-radius: 6px;
  padding: 8px 10px;
  font-size: 13px;
  height: 34px;
  box-sizing: border-box;
  width: 100%;
}
.fgl-input:focus { outline: none; border-color: #4fc3f7; }
.fgl-select {
  background: #252525;
  color: #eee;
  border: 1px solid #3a3a3a;
  border-radius: 6px;
  padding: 4px 8px;
  font-size: 12px;
  height: 30px;
}
.fgl-actions { display: flex; gap: 8px; margin-top: 12px; justify-content: flex-end; }
.fgl-btn {
  background: #2a2a2a;
  color: #fff;
  border: 1px solid #444;
  padding: 8px 16px;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  height: 34px;
}
.fgl-btn:hover { filter: brightness(1.12); }
.fgl-btn:disabled { opacity: 0.45; cursor: not-allowed; }
.fgl-btn.green { background: #2e7d32; border-color: #43a047; }
.fgl-btn.blue { background: #1565c0; border-color: #1e88e5; }
.fgl-btn.red { background: #c62828; border-color: #e53935; }
.fgl-status { color: #4fc3f7; font-size: 12px; margin-top: 8px; text-align: center; }
.fgl-hint { font-size: 11px; color: #888; margin-top: 8px; }
.fgl-running-line { display: flex; align-items: center; gap: 10px; margin-bottom: 8px; font-size: 13px; }
.fgl-running-line span { color: #888; min-width: 56px; }
.fgl-share { flex: 1; max-width: 420px; color: #4fc3f7; }
.fgl-adv { border-top: 1px solid #2a2a2a; background: #161616; flex-shrink: 0; }
.fgl-adv-toggle {
  width: 100%;
  text-align: left;
  padding: 8px 14px;
  background: transparent;
  border: none;
  color: #aaa;
  font-size: 12px;
  cursor: pointer;
}
.fgl-badge {
  margin-left: 6px;
  font-size: 11px;
  color: #e53935;
  border: 1px solid #e53935;
  padding: 0 5px;
  border-radius: 3px;
}
.fgl-adv-body { padding: 6px 10px 10px; max-height: 28vh; overflow-y: auto; }
.fgl-adv-off { color: #888; padding: 8px; font-size: 12px; }
.fgl-menubar {
  background: #1a1a1a !important;
  border: 1px solid #333;
  border-radius: 6px;
  margin-top: 8px;
}
.fgl-bottom {
  flex: 1;
  min-height: 0;
  display: flex;
  gap: 10px;
  padding: 10px 14px 12px;
  background: #121212;
  border-top: 1px solid #2a2a2a;
}
.fgl-peers {
  width: 160px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.fgl-peerbox {
  flex: 1;
  min-height: 80px;
  overflow-y: auto;
  background: #0d0d0d;
  border: 1px solid #2e2e2e;
  border-radius: 8px;
  padding: 8px;
}
.fgl-peer {
  padding: 4px 6px;
  border-radius: 4px;
  margin-bottom: 2px;
  background: #1c1c1c;
  font-size: 12px;
}
.fgl-peer-empty { color: #555; font-size: 12px; }
.fgl-logs-wrap {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.fgl-logbox {
  flex: 1;
  min-height: 90px;
  overflow-y: auto;
  background: #0d0d0d;
  border: 1px solid #2e2e2e;
  border-radius: 8px;
  padding: 8px;
  font-family: Consolas, monospace;
  font-size: 12px;
  color: #d0d0d0;
  margin: 4px 0 6px;
}
.fgl-logline { white-space: pre-wrap; word-break: break-all; }
.fgl-chatrow { display: flex; gap: 6px; flex-shrink: 0; }
.fgl-chatrow .flex1,
.fgl-chatrow .fgl-input { flex: 1; }
.fgl-log-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 6px;
  flex-shrink: 0;
}
.fgl-copy-logs { min-width: 150px; }

.fgl-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.fgl-node-row {
  display: flex !important;
  flex-direction: row !important;
  align-items: flex-end;
  gap: 12px;
  grid-column: 1 / -1;
}
.fgl-node-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.fgl-dhcp {
  display: flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
  font-size: 12px;
  color: #ccc;
  padding-bottom: 8px;
  cursor: pointer;
  flex-shrink: 0;
}
.fgl-adv-note {
  color: #888;
  font-weight: 400;
  font-size: 11px;
  margin-left: 6px;
}
/* log-actions removed */
.fgl-copy-logs { min-width: auto; white-space: nowrap; }

.fgl-logline { white-space: pre-wrap; word-break: break-all; margin: 1px 0; }
.fgl-log-ts { opacity: 0.65; margin-right: 4px; }
.fgl-log-info { color: #d0d0d0; }
.fgl-log-ok { color: #69f0ae; font-weight: 600; }
.fgl-log-join { color: #40c4ff; font-weight: 600; }
.fgl-log-code { color: #ffd54f; font-family: Consolas, monospace; }
.fgl-log-chat { color: #ea80fc; }
.fgl-log-warn { color: #ff8a65; }
/* fangame + placeholder = 2e ligne (moitie / moitie) */



/* Join: LIGNE1 = pseudo + code, LIGNE2 = fangame moitie + reserve */








/* CREATE ligne 1 : 4 champs */


/* JOIN ligne 1 : pseudo col1, code col2, vide a droite (col 3-4) */



/* Ligne 2 commune : fangame = 2 cols (moitie), reserve = 2 cols */





















.fgl-grid-create {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr 1.5fr;
  gap: 10px 12px;
  align-items: end;
}
.fgl-grid-join {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr 1fr;
  gap: 10px 12px;
  align-items: end;
}
.fgl-grid-join .fgl-field-pseudo { grid-column: 1; }
.fgl-grid-join .fgl-field-code { grid-column: 2; }
.fgl-grid-create .fgl-field-half,
.fgl-grid-join .fgl-field-half {
  grid-column: span 2;
}
.fgl-fangame-row {
  display: flex;
  gap: 8px;
  align-items: center;
  width: 100%;
}
.fgl-fangame-row .fgl-input { flex: 1 1 auto; min-width: 0; }
.fgl-fangame-row .fgl-btn { flex: 0 0 auto; }
.fgl-fangame-title {
  color: #69f0ae;
  font-weight: 700;
  font-size: 14px;
  line-height: 34px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>