<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { resultsApi, hostsApi } from '../api'
import { formatTime } from '../time'

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
})

async function loadResults() {
  loading.value = true
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

function parseEvidence(e: string) {
  try { return JSON.parse(e).join(', ') } catch { return e }
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
        </select>
      </div>
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
              </tr>
            </thead>
            <tbody>
              <tr v-for="r in results" :key="r.id">
                <td style="font-weight: 600;">VM {{ r.vmid }}</td>
                <td>{{ hostMap[r.host_id] || `#${r.host_id}` }}</td>
                <td><span :class="['badge', statusBadge(r.status)]">{{ statusLabel(r.status) }}</span></td>
                <td><code style="font-size: 12px;">{{ r.method || '-' }}</code></td>
                <td style="font-size: 13px; max-width: 300px; word-break: break-all;">
                  {{ parseEvidence(r.evidence || '[]') || '-' }}
                </td>
                <td style="font-size: 13px; color: var(--text-secondary);">{{ formatTime(r.first_seen) }}</td>
                <td style="font-size: 13px; color: var(--text-secondary);">{{ formatTime(r.last_seen) }}</td>
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
  </div>
</template>

<style scoped>
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
