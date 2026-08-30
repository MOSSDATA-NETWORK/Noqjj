<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { resultsApi, hostsApi } from '../api'

const results = ref<any[]>([])
const hosts = ref<any[]>([])
const loading = ref(true)
const filterHost = ref<number | null>(null)
const filterStatus = ref('')

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

function statusBadge(s: string) {
  const m: Record<string, string> = {
    detected: 'badge-detected', confirmed: 'badge-confirmed',
    cleaned: 'badge-cleaned', clean: 'badge-clean',
  }
  return m[s] || 'badge-unknown'
}

function statusLabel(s: string) {
  const m: Record<string, string> = {
    detected: '新发现', confirmed: '持续存在', cleaned: '已清除', clean: '正常',
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

function formatTime(t: string) {
  if (!t) return '-'
  return new Date(t).toLocaleString('zh-CN')
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
        </select>
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
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
