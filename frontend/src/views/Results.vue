<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { resultsApi, hostsApi } from '../api'
import { formatTime } from '../time'
import axios from 'axios'

const results = ref<any[]>([])
const hosts = ref<any[]>([])
const loading = ref(true)
const filterHost = ref<number | null>(null)
const filterStatus = ref('')
const scanningVmid = ref<string | null>(null)

onMounted(async () => {
  try {
    const [r, h] = await Promise.all([resultsApi.list(), hostsApi.list()])
    if (r.ok) results.value = r.data
    if (h.ok) hosts.value = h.data
  } finally {
    loading.value = false
  }
})

async function loadResults() {
  loading.value = true
  try {
    const res = await resultsApi.list(filterHost.value || undefined)
    if (res.ok) results.value = res.data
  } finally {
    loading.value = false
  }
}

function filteredResults() {
  let r = results.value
  if (filterStatus.value) r = r.filter(x => x.status === filterStatus.value)
  return r
}

function needsDiskScanCount() {
  return results.value.filter(x => x.status === 'needs_disk_scan').length
}

function statusBadge(s: string) {
  const m: Record<string, string> = {
    detected: 'badge-detected', confirmed: 'badge-confirmed',
    cleaned: 'badge-cleaned', clean: 'badge-clean',
    needs_disk_scan: 'badge-confirmed',
  }
  return m[s] || 'badge-unknown'
}

function statusLabel(s: string) {
  const m: Record<string, string> = {
    detected: '新发现', confirmed: '持续存在', cleaned: '已清除', clean: '正常',
    needs_disk_scan: '待检测',
  }
  return m[s] || s
}

function hostName(id: number) {
  const h = hosts.value.find(x => x.id === id)
  return h ? h.name : `#${id}`
}

function parseEvidence(e: string) {
  try { return JSON.parse(e).join(', ') } catch { return e }
}

// formatTime 从 ../time 导入

async function triggerDiskScan(vmid: string) {
  if (!confirm(`确定要对 VM ${vmid} 执行磁盘扫描？\n\n这会复制磁盘镜像并挂载检查，可能需要几分钟。`)) return
  scanningVmid.value = vmid
  try {
    // 找到该 VM 所属的主机
    const result = results.value.find(r => r.vmid === vmid)
    const hostId = result?.host_id
    if (!hostId) { alert('未找到主机信息'); return }

    const res = await axios.post(`/api/hosts/${hostId}/scan-vm`, { vmid })
    if (res.data.ok) {
      alert(`VM ${vmid} 磁盘扫描已启动，完成后结果会自动更新`)
      // 刷新结果
      setTimeout(() => loadResults(), 30000)
    } else {
      alert(res.data.error || '扫描失败')
    }
  } catch (e: any) {
    alert(e.response?.data?.error || '请求失败')
  } finally {
    scanningVmid.value = null
  }
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
        <select class="form-input" style="width: 160px;" v-model="filterHost" @change="loadResults">
          <option :value="null">全部主机</option>
          <option v-for="h in hosts" :key="h.id" :value="h.id">{{ h.name }}</option>
        </select>
        <select class="form-input" style="width: 140px;" v-model="filterStatus">
          <option value="">全部状态</option>
          <option value="detected">新发现</option>
          <option value="confirmed">持续存在</option>
          <option value="cleaned">已清除</option>
          <option value="needs_disk_scan">待检测</option>
        </select>
      </div>
    </div>

    <!-- 磁盘扫描提示 -->
    <div v-if="needsDiskScanCount() > 0" class="card" style="margin-bottom: 16px; background: rgba(255,149,0,0.04); border: 1px solid rgba(255,149,0,0.15);">
      <div style="display: flex; align-items: center; gap: 12px;">
        <span style="font-size: 24px;">💾</span>
        <div style="flex: 1;">
          <div style="font-weight: 600;">{{ needsDiskScanCount() }} 个 VM 未安装 Guest Agent</div>
          <div style="font-size: 13px; color: var(--text-secondary); margin-top: 2px;">
            这些 VM 无法通过 GA 检测，需要复制磁盘镜像挂载后扫描文件系统
          </div>
        </div>
      </div>
    </div>

    <div class="card">
      <div v-if="loading" style="text-align: center; padding: 40px;">
        <div class="spinner" style="margin: 0 auto;"></div>
      </div>
      <div v-else-if="filteredResults().length === 0" class="empty-state">
        <p>暂无检测结果</p>
      </div>
      <div v-else class="table-container">
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
            <tr v-for="r in filteredResults()" :key="r.id">
              <td style="font-weight: 600;">VM {{ r.vmid }}</td>
              <td>{{ hostName(r.host_id) }}</td>
              <td><span :class="['badge', statusBadge(r.status)]">{{ statusLabel(r.status) }}</span></td>
              <td>
                <code style="font-size: 12px;">{{ r.method || '-' }}</code>
              </td>
              <td style="font-size: 13px; max-width: 300px; word-break: break-all;">
                {{ parseEvidence(r.evidence || '[]') || '-' }}
              </td>
              <td style="font-size: 13px; color: var(--text-secondary);">{{ formatTime(r.first_seen) }}</td>
              <td style="font-size: 13px; color: var(--text-secondary);">{{ formatTime(r.last_seen) }}</td>
              <td>
                <button v-if="r.status === 'needs_disk_scan'"
                  class="btn btn-sm btn-secondary"
                  @click="triggerDiskScan(r.vmid)"
                  :disabled="scanningVmid === r.vmid">
                  {{ scanningVmid === r.vmid ? '扫描中...' : '磁盘扫描' }}
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
