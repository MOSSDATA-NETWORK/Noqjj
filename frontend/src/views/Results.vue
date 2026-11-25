<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { resultsApi, hostsApi, diskScanApi } from '../api'
import { formatTime, formatDate } from '../time'

const results = ref<any[]>([])
const hosts = ref<any[]>([])
const loading = ref(true)
const total = ref(0)
const pageSize = ref(20)
const currentPage = ref(1)
const filterHost = ref<number | null>(null)
const filterStatus = ref('')
const totalPages = computed(() => Math.max(1, Math.ceil(total.value / pageSize.value)))
const hostMap = computed(() => {
  const map: Record<number, string> = {}
  hosts.value.forEach((h: any) => { map[h.id] = h.name })
  return map
})
const pageOptions = [10, 20, 50, 100]

onMounted(async () => {
  try {
    const h = await hostsApi.list()
    if (h.ok) hosts.value = h.data
  } catch {}
  await loadResults()
  pollLoop()
})

onUnmounted(() => {
  disposed = true
  clearTimeout(pollTimer)
})

async function loadResults(silent = false) {
  if (!silent) loading.value = true
  try {
    const offset = (currentPage.value - 1) * pageSize.value
    const params: any = { limit: pageSize.value, offset }
    if (filterHost.value) params.host_id = filterHost.value
    if (filterStatus.value) params.status = filterStatus.value
    const res = await resultsApi.list(params)
    if (res.ok) {
      results.value = res.data
      total.value = res.total || 0
    }
  } finally {
    loading.value = false
  }
}

function changePage(page: number) {
  if (page < 1 || page > totalPages.value) return
  currentPage.value = page
  loadResults()
}

function changePageSize(size: number) {
  pageSize.value = size
  currentPage.value = 1
  loadResults()
}

function onFilterChange() {
  currentPage.value = 1
  loadResults()
}

function statusBadge(s: string) {
  const m: Record<string, string> = {
    detected: 'badge-detected', confirmed: 'badge-confirmed',
    cleaned: 'badge-cleaned', clean: 'badge-clean',
    needs_disk_scan: 'badge-confirmed', error: 'badge-detected',
  }
  return m[s] || 'badge-unknown'
}

function statusLabel(s: string) {
  const m: Record<string, string> = {
    detected: '新发现', confirmed: '持续存在', cleaned: '已清除', clean: '正常',
    needs_disk_scan: '待检测', error: '检测失败',
  }
  return m[s] || s
}

// 证据代码 → 中文说明（与后端 chicken-check.sh 的输出一一对应）
const EVIDENCE_FIXED: Record<string, { label: string; desc: string }> = {
  incus_dir: { label: 'Incus 切鸡目录', desc: '存在 /opt/incus 目录——安装了 Incus 容器系统，是切小鸡的核心组件' },
  incus_data: { label: 'Incus 数据目录', desc: '存在 /var/lib/incus——Incus 已初始化并实际使用过' },
  incushlii_agent: { label: 'incushlii 管理程序', desc: '存在 /usr/local/bin/incushlii-agent——shlii 平台的切鸡管理程序' },
  lxd: { label: 'LXD 运营痕迹', desc: 'LXD 已初始化（lxd.db / database / 非空容器或磁盘）——在用 LXD 分割实例' },
  nodehatch_proj: { label: 'NodeHatch 面板项目', desc: 'Incus 中存在 NodeHatch 切鸡面板创建的项目' },
  nodehatch_cert: { label: 'NodeHatch 面板证书', desc: 'Incus 信任证书中存在 NodeHatch 面板的证书' },
  zabbly_incus: { label: 'zabbly 软件源', desc: '配置了 zabbly 的 Incus 安装源——专门用于装 Incus 切鸡' },
  api_8443: { label: 'Incus 管理端口', desc: 'incusd 进程正在监听 8443 管理端口' },
  podman_split: { label: 'Podman 容器分裂', desc: 'Podman 运行多个容器、有 sing-box/xray 代理核心在跑，且呈现批量开实例特征：同一镜像 ≥3 个容器，或 ≥2 个容器开放 SSH(22) 端口——容器化分销（NAT VPS 分销）形态。自用的代理+监控等各异镜像容器不计；关机后的磁盘检测拿不到端口，只看同镜像数量' },
  panel_run: { label: '分销面板运营痕迹', desc: '系统服务/代理配置文件中出现分销面板品牌（narwhalcloud / fuckip）——分销运营实锤' },
  vm_stopped: { label: '已关机', desc: '检测时该 VM 处于关机状态，未执行检测' },
}

interface EvidenceItem { raw: string; label: string; desc: string; items?: string[] }

// 把后端证据串转成中文条目列表。
// 格式: "svc:8 hist:1 net:1##svc=名1|名2##hist=命令##net=连接"
// '##' 后是明细段（检测脚本 v6 起上报），旧数据无明细段则只显示数量
function evidenceItems(e: string | null | undefined): EvidenceItem[] {
  const parts = (e || '').split('##')
  const head = (parts.shift() || '').trim()
  if (!head) return []
  const detailMap: Record<string, string[]> = {}
  for (const sec of parts) {
    const i = sec.indexOf('=')
    if (i <= 0) continue
    const key = sec.slice(0, i)
    const vals = sec.slice(i + 1).split(',').map(s => s.trim()).filter(Boolean)
    detailMap[key] = [...(detailMap[key] || []), ...vals]
  }
  return head.split(/\s+/).map((t): EvidenceItem => {
    const m = t.match(/^(svc|hist|net|jeeyio|panel_hist):(\d+)$/)
    if (m) {
      const n = Number(m[2])
      const items = detailMap[m[1]]
      if (m[1] === 'svc') return { raw: t, label: `可疑系统服务 × ${n}`, desc: `在 /etc/systemd/system/ 下发现 ${n} 个名称含 incus / shlii / nodehatch 的服务文件——安装了切鸡或机场相关服务`, items }
      if (m[1] === 'hist') return { raw: t, label: `可疑命令历史 × ${n}`, desc: `bash 历史中有 ${n} 条与 shlii.io / incushlii / nodehatch / jeeyio 相关的命令——执行过安装、管理或访问操作`, items }
      if (m[1] === 'jeeyio') return { raw: t, label: `jeeyio 域名引用 × ${n}`, desc: `发现 ${n} 处 jeeyio.com / jeeyio.net 的域名引用（进程命令行 / 配置文件 / 命令历史）——只按域名匹配，不按 IP（其 IP 为 Cloudflare 共享地址，按 IP 会误判）；不扫二进制文件（含数据库）和 .log 日志（代理访问记录里的域名只代表有人访问过该网站）`, items }
      if (m[1] === 'panel_hist') return { raw: t, label: `分销面板域名提及 × ${n}`, desc: `bash 历史中出现 ${n} 次分销面板域名（narwhalcloud / fuckip）——弱信号：可能只是测评机 ping 商家测试点，结合下方明细人工判断`, items }
      return { raw: t, label: `可疑网络连接 × ${n}`, desc: `当前有 ${n} 条由可疑进程发起的网络连接——切鸡 / 机场程序正在联网运行`, items }
    }
    const fixed = EVIDENCE_FIXED[t]
    // 无数量的固定特征（panel_run/podman_split 等）也可能带明细段，一并挂上
    const items = detailMap[t]
    const base = fixed ? { raw: t, ...fixed } : { raw: t, label: t, desc: '' }
    return items && items.length ? { ...base, items } : base
  })
}

function methodLabel(m: string | null | undefined) {
  const map: Record<string, string> = { ga: 'GA 内部检测', disk: '磁盘扫描' }
  return map[m || ''] || m || '-'
}

const detailRow = ref<any>(null)
const detailHistory = ref<any[]>([])
const historyLoading = ref(false)

async function openDetail(r: any) {
  detailRow.value = r
  detailHistory.value = []
  historyLoading.value = true
  try {
    const res = await resultsApi.history(r.id)
    if (res.ok) detailHistory.value = res.data || []
  } catch {} finally {
    historyLoading.value = false
  }
}

const diskResult = ref<{ ok: boolean; text: string } | null>(null)
const confirmBox = ref<{ title: string; message: string; confirmText: string; action: () => void } | null>(null)

function askConfirm(title: string, message: string, confirmText: string, action: () => void) {
  confirmBox.value = { title, message, confirmText, action }
}

function runConfirm() {
  const action = confirmBox.value?.action
  confirmBox.value = null
  action?.()
}

// ---- 磁盘扫描：任务在服务器后台队列执行，切走/关闭页面不影响；这里只轮询展示进度 ----
const diskJobs = ref<any[]>([])        // 进行中的任务
const diskTasks = ref<any[]>([])       // 排队中/扫描中的 VM
const lastJob = ref<any>(null)         // 最近一次结束的任务
const needsTotal = ref(0)              // 待检测 VM 数
const needsWhitelisted = ref(0)        // 其中在白名单内的
const needsPending = computed(() => Math.max(0, needsTotal.value - needsWhitelisted.value))
const lastDismissed = ref(Number(localStorage.getItem('diskScanLastDismissed')) || 0)
const failList = ref<any[] | null>(null)
let pollTimer: number | undefined
let pollSeq = 0
let disposed = false
let lastSig = ''

const taskMap = computed(() => {
  const m: Record<string, string> = {}
  diskTasks.value.forEach((t: any) => { m[`${t.host_id}:${t.vmid}`] = t.status })
  return m
})

function taskState(r: any): string {
  return taskMap.value[`${r.host_id}:${r.vmid}`] || ''
}

// 可手动磁盘扫描：待检测、命中（GA 不可用时只能靠磁盘复查）、检测失败、上次就是磁盘扫描的、白名单内的
function canDiskScan(r: any): boolean {
  return ['needs_disk_scan', 'detected', 'confirmed', 'error'].includes(r.status) || r.method === 'disk' || !!r.wl_active
}

async function pollDisk() {
  const seq = ++pollSeq
  try {
    const res = await diskScanApi.active()
    if (seq !== pollSeq || !res.ok) return  // 已有更新的请求发出：丢弃迟到的旧响应，避免进度回退
    const d = res.data
    diskJobs.value = d.jobs || []
    diskTasks.value = d.tasks || []
    lastJob.value = d.last
    needsTotal.value = d.needs_total || 0
    needsWhitelisted.value = d.needs_whitelisted || 0
    // 有 VM 扫完或任务结束（含停止后在途那台扫完）→ 静默刷新列表
    const sig = diskJobs.value.map((j: any) => `${j.id}:${j.done}`).join(',') + '|' + (d.last ? `${d.last.id}:${d.last.done}` : '')
    if (lastSig && sig !== lastSig) loadResults(true)
    lastSig = sig
  } catch {}
}

async function pollLoop() {
  await pollDisk()
  if (disposed) return
  clearTimeout(pollTimer)  // 保证只有一个待执行的定时器（提交操作后会立即重启轮询）
  pollTimer = window.setTimeout(pollLoop, diskJobs.value.length ? 3000 : 10000)
}

function sourceLabel(s: string) {
  return ({ single: '单台', batch: '批量', schedule: '定时自动' } as Record<string, string>)[s] || s
}

function jobEndLabel(s: string) {
  return ({ completed: '已完成', stopped: '已停止', interrupted: '服务重启中断' } as Record<string, string>)[s] || s
}

function runningText(j: any) {
  const names = diskTasks.value
    .filter((t: any) => t.job_id === j.id && t.status === 'running')
    .map((t: any) => `VM ${t.vmid}（${hostMap.value[t.host_id] || '#' + t.host_id}）`)
  return names.length ? `正在扫描：${names.join('、')}` : '排队中'
}

function jobPercent(j: any) {
  return j.total ? Math.round(((j.done + j.skipped) / j.total) * 100) : 0
}

async function submit(p: Promise<any>) {
  try {
    const res = await p
    diskResult.value = { ok: !!res.ok, text: res.ok ? (res.message || '已提交') : (res.error || '操作失败') }
    return !!res.ok
  } catch (e: any) {
    diskResult.value = { ok: false, text: e.response?.data?.error || '请求失败' }
    return false
  } finally {
    pollLoop()  // 立即刷新并切到 3 秒轮询
  }
}

function batchDiskScan() {
  const skip = needsWhitelisted.value ? `（白名单内 ${needsWhitelisted.value} 台跳过）` : ''
  askConfirm('批量磁盘扫描',
    `将对 ${needsPending.value} 台待检测 VM 执行磁盘扫描${skip}。\n任务在服务器后台执行，可以离开本页面或关闭浏览器；同一宿主机逐台扫描，不同宿主机并行。`,
    '开始扫描',
    () => { submit(diskScanApi.batch()) })
}

function triggerDiskScan(r: any) {
  askConfirm('磁盘扫描',
    `确定对 VM ${r.vmid} 执行磁盘扫描？\n将只读挂载磁盘镜像逐项检查切鸡特征，约几秒到几分钟。任务在服务器后台执行，可以离开本页面。`,
    '开始扫描',
    () => { submit(diskScanApi.scanVm(r.host_id, r.vmid)) })
}

function stopJob(j: any) {
  askConfirm('停止磁盘扫描',
    `停止后，尚未开始的 VM 不再扫描；正在扫描的那台会扫完。`,
    '停止',
    () => { submit(diskScanApi.stop(j.id)) })
}

async function toggleWhitelist(r: any) {
  const ok = await submit(r.wl_active
    ? diskScanApi.whitelistRemove(r.host_id, r.vmid)
    : diskScanApi.whitelistAdd(r.host_id, r.vmid))
  if (ok) loadResults(true)
}

function dismissLast() {
  if (!lastJob.value) return
  lastDismissed.value = lastJob.value.id
  localStorage.setItem('diskScanLastDismissed', String(lastJob.value.id))
}

async function showFailures(j: any) {
  try {
    const res = await diskScanApi.get(j.id)
    if (res.ok) failList.value = (res.data.tasks || []).filter((t: any) => t.status === 'error' || t.status === 'cancelled')
  } catch {}
}

function getPageNumbers() {
  const pages: (number | string)[] = []
  const p = currentPage.value
  const t = totalPages.value
  if (t <= 7) {
    for (let i = 1; i <= t; i++) pages.push(i)
  } else {
    pages.push(1)
    if (p > 3) pages.push('...')
    for (let i = Math.max(2, p - 1); i <= Math.min(t - 1, p + 1); i++) pages.push(i)
    if (p < t - 2) pages.push('...')
    pages.push(t)
  }
  return pages
}
</script>

<template>
  <div>
    <div class="page-header" style="display: flex; justify-content: space-between; align-items: flex-start;">
      <div>
        <h1 class="page-title">检测结果</h1>
        <p class="page-subtitle">查看所有 VM 的检测状态</p>
      </div>
      <div style="display: flex; gap: 12px;">
        <select class="form-input" style="width: 160px;" v-model="filterHost" @change="onFilterChange">
          <option :value="null">全部主机</option>
          <option v-for="h in hosts" :key="h.id" :value="h.id">{{ h.name }}</option>
        </select>
        <select class="form-input" style="width: 140px;" v-model="filterStatus" @change="onFilterChange">
          <option value="">全部状态</option>
          <option value="detected">新发现</option>
          <option value="confirmed">持续存在</option>
          <option value="cleaned">已清除</option>
          <option value="needs_disk_scan">待检测</option>
          <option value="error">检测失败</option>
        </select>
      </div>
    </div>

    <!-- 无 Guest Agent 提示条 -->
    <div v-if="needsTotal > 0" class="card" style="margin-bottom: 16px; background: rgba(255,149,0,0.04); border: 1px solid rgba(255,149,0,0.15);">
      <div style="display: flex; align-items: center; gap: 12px; flex-wrap: wrap;">
        <span style="font-size: 24px;">💾</span>
        <div style="flex: 1; min-width: 200px;">
          <div style="font-weight: 600;">{{ needsTotal }} 个 VM 未安装 Guest Agent</div>
          <div style="font-size: 13px; color: var(--text-secondary); margin-top: 2px;">
            这些 VM 无法通过 GA 检测，可逐台或批量执行磁盘扫描（服务器后台执行，可离开本页面）<template v-if="needsWhitelisted">；其中 {{ needsWhitelisted }} 台在白名单内，批量与定时扫描会跳过</template>
          </div>
        </div>
        <button class="btn btn-primary" @click="batchDiskScan" :disabled="needsPending === 0">批量磁盘扫描</button>
      </div>
    </div>

    <!-- 进行中的磁盘扫描任务 -->
    <div v-for="j in diskJobs" :key="j.id" class="card" style="margin-bottom: 16px; border: 1px solid rgba(0,122,255,0.18); background: rgba(0,122,255,0.03);">
      <div style="display: flex; align-items: center; gap: 12px; flex-wrap: wrap;">
        <div class="spinner" style="width: 20px; height: 20px; flex-shrink: 0;"></div>
        <div style="flex: 1; min-width: 220px;">
          <div style="font-weight: 600;">磁盘扫描进行中（{{ sourceLabel(j.source) }}）：{{ j.done }} / {{ j.total }}</div>
          <div class="job-progress"><div :style="{ width: jobPercent(j) + '%' }"></div></div>
          <div style="font-size: 13px; color: var(--text-secondary);">
            命中 {{ j.found }} · 失败 {{ j.failed }} · {{ runningText(j) }}
          </div>
        </div>
        <button class="btn btn-sm btn-secondary" @click="stopJob(j)">停止</button>
      </div>
    </div>

    <!-- 最近一次磁盘扫描结果 -->
    <div v-if="!diskJobs.length && lastJob && lastJob.id !== lastDismissed"
      style="margin-bottom: 16px; display: flex; align-items: center; gap: 10px; padding: 12px 16px; border-radius: 12px; flex-wrap: wrap;"
      :style="{ background: lastJob.failed || lastJob.skipped ? 'rgba(255,149,0,0.06)' : 'rgba(52,199,89,0.08)', border: '1px solid ' + (lastJob.failed || lastJob.skipped ? 'rgba(255,149,0,0.2)' : 'rgba(52,199,89,0.2)') }">
      <span style="font-size: 18px;">{{ lastJob.found ? '🚨' : (lastJob.failed || lastJob.skipped ? '⚠️' : '✅') }}</span>
      <span style="font-size: 14px; flex: 1; min-width: 240px;">
        最近一次磁盘扫描（{{ sourceLabel(lastJob.source) }}，{{ jobEndLabel(lastJob.status) }}，{{ formatTime(lastJob.completed_at) }}）：
        共 {{ lastJob.total }} 台，干净 {{ lastJob.done - lastJob.found - lastJob.failed }}，命中 {{ lastJob.found }}，失败 {{ lastJob.failed }}<template v-if="lastJob.skipped">，未执行 {{ lastJob.skipped }}</template>
      </span>
      <button v-if="lastJob.failed || lastJob.skipped" class="btn btn-sm btn-secondary" @click="showFailures(lastJob)">查看未成功</button>
      <button class="btn btn-sm btn-secondary" @click="dismissLast">关闭</button>
    </div>

    <!-- 磁盘扫描结果条 -->
    <div v-if="diskResult" style="margin-bottom: 16px; display: flex; align-items: center; gap: 10px; padding: 12px 16px; border-radius: 12px;"
      :style="{ background: diskResult.ok ? 'rgba(52,199,89,0.08)' : 'rgba(255,59,48,0.08)', border: '1px solid ' + (diskResult.ok ? 'rgba(52,199,89,0.2)' : 'rgba(255,59,48,0.2)') }">
      <span style="font-size: 18px;">{{ diskResult.ok ? '✅' : '❌' }}</span>
      <span style="font-size: 14px; flex: 1;">{{ diskResult.text }}</span>
      <button class="btn btn-sm btn-secondary" @click="diskResult = null">关闭</button>
    </div>

    <div class="card">
      <div v-if="loading" style="text-align: center; padding: 40px;">
        <div class="spinner" style="margin: 0 auto;"></div>
      </div>
      <div v-else-if="results.length === 0" class="empty-state">
        <p>暂无检测结果</p>
      </div>
      <div v-else>
        <div class="table-container">
          <table>
            <thead>
              <tr>
                <th>VM ID</th>
                <th>主机</th>
                <th>状态</th>
                <th>检测方式</th>
                <th>证据</th>
                <th>首次发现</th>
                <th>最后检测</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="r in results" :key="r.id">
                <td style="font-weight: 600;">VM {{ r.vmid }}</td>
                <td>{{ hostMap[r.host_id] || `#${r.host_id}` }}</td>
                <td>
                  <span :class="['badge', statusBadge(r.status)]">{{ statusLabel(r.status) }}</span>
                  <span v-if="r.wl_active" class="wl-badge"
                    :title="(r.wl_source === 'manual' ? '手动加入' : '磁盘扫描连续干净自动加入') + '，' + formatDate(r.wl_expires_at) + ' 失效；批量与定时磁盘扫描跳过'">
                    白名单·至{{ formatDate(r.wl_expires_at).replace(/^\d{4}\//, '') }}
                  </span>
                </td>
                <td><code style="font-size: 12px;">{{ r.method || '-' }}</code></td>
                <td style="font-size: 13px; max-width: 300px;">
                  <div v-if="evidenceItems(r.evidence).length" style="display: flex; flex-wrap: wrap; gap: 6px; align-items: center;">
                    <span v-for="t in evidenceItems(r.evidence)" :key="t.raw" class="evidence-tag">{{ t.label }}</span>
                    <button class="btn btn-sm btn-secondary" @click="openDetail(r)">详情</button>
                  </div>
                  <span v-else style="color: var(--text-tertiary);">-</span>
                </td>
                <td style="font-size: 13px; color: var(--text-secondary);">{{ formatTime(r.first_seen) }}</td>
                <td style="font-size: 13px; color: var(--text-secondary);">{{ formatTime(r.last_seen) }}</td>
                <td>
                  <div style="display: flex; gap: 6px; flex-wrap: wrap;">
                    <button v-if="canDiskScan(r)"
                      class="btn btn-sm btn-secondary"
                      @click="triggerDiskScan(r)"
                      :disabled="!!taskState(r)">
                      {{ taskState(r) === 'running' ? '扫描中...' : taskState(r) === 'pending' ? '排队中...' : '磁盘扫描' }}
                    </button>
                    <button v-if="r.wl_active || r.status === 'needs_disk_scan'"
                      class="btn btn-sm btn-secondary"
                      @click="toggleWhitelist(r)">
                      {{ r.wl_active ? '移出白名单' : '加入白名单' }}
                    </button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <!-- 分页栏 -->
        <div class="pagination">
          <div class="pagination-info">
            共 {{ total }} 条，第 {{ currentPage }}/{{ totalPages }} 页
          </div>
          <div class="pagination-controls">
            <select class="page-size-select" :value="pageSize" @change="changePageSize(Number(($event.target as HTMLSelectElement).value))">
              <option v-for="opt in pageOptions" :key="opt" :value="opt">{{ opt }} 条/页</option>
            </select>
            <div class="page-buttons">
              <button class="page-btn" :disabled="currentPage <= 1" @click="changePage(1)">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><polyline points="11 17 6 12 11 7"/><polyline points="18 17 13 12 18 7"/></svg>
              </button>
              <button class="page-btn" :disabled="currentPage <= 1" @click="changePage(currentPage - 1)">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><polyline points="15 18 9 12 15 6"/></svg>
              </button>
              <template v-for="(p, i) in getPageNumbers()" :key="i">
                <span v-if="p === '...'" class="page-ellipsis">...</span>
                <button v-else class="page-btn" :class="{ active: p === currentPage }" @click="changePage(Number(p))">
                  {{ p }}
                </button>
              </template>
              <button class="page-btn" :disabled="currentPage >= totalPages" @click="changePage(currentPage + 1)">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><polyline points="9 18 15 12 9 6"/></svg>
              </button>
              <button class="page-btn" :disabled="currentPage >= totalPages" @click="changePage(totalPages)">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><polyline points="13 17 18 12 13 7"/><polyline points="6 17 11 12 6 7"/></svg>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 磁盘扫描未成功明细 -->
    <div v-if="failList" class="modal-overlay" @click.self="failList = null">
      <div class="modal" style="max-width: 560px;">
        <div class="modal-header">未成功的 VM（{{ failList.length }} 台）</div>
        <div class="modal-body" style="max-height: 420px; overflow-y: auto;">
          <div v-for="t in failList" :key="t.id"
            style="display: flex; gap: 10px; padding: 8px 0; border-bottom: 1px dashed var(--border); font-size: 13px;">
            <span style="font-weight: 600; white-space: nowrap;">VM {{ t.vmid }}</span>
            <span style="color: var(--text-secondary); white-space: nowrap;">{{ hostMap[t.host_id] || `#${t.host_id}` }}</span>
            <span style="color: var(--text-secondary); word-break: break-all;">{{ t.status === 'cancelled' ? '未执行：' : '' }}{{ t.detail || '-' }}</span>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="failList = null">关闭</button>
        </div>
      </div>
    </div>

    <!-- 确认弹窗 -->
    <div v-if="confirmBox" class="modal-overlay" @click.self="confirmBox = null">
      <div class="modal" style="max-width: 440px;">
        <div class="modal-header">{{ confirmBox.title }}</div>
        <div class="modal-body">
          <p style="font-size: 14px; color: var(--text-secondary); line-height: 1.8; margin: 0; white-space: pre-line;">{{ confirmBox.message }}</p>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="confirmBox = null">取消</button>
          <button class="btn btn-primary" @click="runConfirm">{{ confirmBox.confirmText }}</button>
        </div>
      </div>
    </div>

    <!-- 检测详情弹窗 -->
    <div v-if="detailRow" class="modal-overlay" @click.self="detailRow = null">
      <div class="modal" style="max-width: 560px;">
        <div class="modal-header">VM {{ detailRow.vmid }} 检测详情</div>
        <div class="modal-body">
          <div style="display: flex; flex-wrap: wrap; gap: 16px; margin-bottom: 14px; font-size: 13px; color: var(--text-secondary);">
            <span>状态：<b style="color: var(--text);">{{ statusLabel(detailRow.status) }}</b></span>
            <span>检测方式：{{ methodLabel(detailRow.method) }}</span>
            <span>主机：{{ hostMap[detailRow.host_id] || `#${detailRow.host_id}` }}</span>
          </div>
          <div v-if="evidenceItems(detailRow.evidence).length">
            <div v-for="t in evidenceItems(detailRow.evidence)" :key="t.raw"
              style="padding: 10px 14px; border: 1px solid var(--border); border-radius: 10px; margin-bottom: 8px;">
              <div style="font-weight: 600; font-size: 14px;">{{ t.label }}</div>
              <div style="font-size: 13px; color: var(--text-secondary); margin-top: 3px; line-height: 1.6;">
                {{ t.desc || ('原始证据：' + t.raw) }}
              </div>
              <ul v-if="t.items && t.items.length" style="margin: 6px 0 0; padding-left: 18px;">
                <li v-for="(it, i) in t.items" :key="i"
                  style="font-size: 12.5px; color: var(--text-secondary); line-height: 1.7; word-break: break-all;">{{ it }}</li>
              </ul>
            </div>
          </div>
          <p v-else style="color: var(--text-secondary); font-size: 14px; margin: 0;">没有命中任何切鸡特征。</p>

          <!-- 状态变迁历史（新发现/已清除/再次发现，含每次的证据快照） -->
          <div style="margin-top: 16px; border-top: 1px solid var(--border); padding-top: 14px;">
            <div style="font-weight: 600; font-size: 14px; margin-bottom: 10px;">历史记录</div>
            <div v-if="historyLoading" style="color: var(--text-tertiary); font-size: 13px;">加载中...</div>
            <div v-else-if="!detailHistory.length" style="color: var(--text-tertiary); font-size: 13px;">暂无历史记录</div>
            <div v-else style="max-height: 260px; overflow-y: auto;">
              <div v-for="h in detailHistory" :key="h.id"
                style="display: flex; gap: 10px; padding: 8px 0; border-bottom: 1px dashed var(--border); font-size: 13px;">
                <span style="color: var(--text-secondary); white-space: nowrap;">{{ formatTime(h.created_at) }}</span>
                <span :class="['badge', statusBadge(h.status)]" style="flex-shrink: 0;">{{ statusLabel(h.status) }}</span>
                <span style="color: var(--text-tertiary); white-space: nowrap;">{{ methodLabel(h.method) }}</span>
                <span style="display: flex; flex-wrap: wrap; gap: 4px;">
                  <span v-for="t in evidenceItems(h.evidence)" :key="t.raw" class="evidence-tag">{{ t.label }}</span>
                </span>
              </div>
            </div>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="detailRow = null">关闭</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.evidence-tag {
  display: inline-block;
  padding: 2px 8px;
  border-radius: 6px;
  font-size: 12px;
  background: rgba(255, 59, 48, 0.08);
  color: var(--red);
  border: 1px solid rgba(255, 59, 48, 0.2);
  white-space: nowrap;
}
.wl-badge {
  display: block;
  width: fit-content;
  margin-top: 4px;
  padding: 1px 7px;
  border-radius: 6px;
  font-size: 11.5px;
  background: rgba(0, 122, 255, 0.08);
  color: var(--accent);
  border: 1px solid rgba(0, 122, 255, 0.2);
  white-space: nowrap;
}
.job-progress {
  height: 6px;
  margin: 8px 0 6px;
  border-radius: 3px;
  background: rgba(0, 0, 0, 0.06);
  overflow: hidden;
}
.job-progress > div {
  height: 100%;
  background: var(--accent);
  transition: width 0.4s;
}
.pagination {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 0 4px;
  flex-wrap: wrap;
  gap: 12px;
}
.pagination-info { font-size: 13px; color: var(--text-secondary); }
.pagination-controls { display: flex; align-items: center; gap: 12px; }
.page-size-select {
  padding: 6px 10px; border: 1px solid var(--border); border-radius: 8px;
  font-size: 13px; background: var(--bg-secondary); color: var(--text);
  cursor: pointer; font-family: var(--font);
}
.page-buttons { display: flex; align-items: center; gap: 4px; }
.page-btn {
  display: inline-flex; align-items: center; justify-content: center;
  min-width: 32px; height: 32px; padding: 0 6px;
  border: 1px solid var(--border); border-radius: 8px;
  background: var(--bg-secondary); color: var(--text);
  font-size: 13px; font-weight: 500; cursor: pointer;
  transition: all 0.15s; font-family: var(--font);
}
.page-btn:hover:not(:disabled):not(.active) { background: rgba(0,0,0,0.04); }
.page-btn.active { background: var(--accent); color: white; border-color: var(--accent); }
.page-btn:disabled { opacity: 0.35; cursor: not-allowed; }
.page-ellipsis { padding: 0 4px; color: var(--text-tertiary); font-size: 13px; }
@media (max-width: 768px) {
  .pagination { flex-direction: column; align-items: stretch; }
  .pagination-controls { justify-content: space-between; }
}
</style>
