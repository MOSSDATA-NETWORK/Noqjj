<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { scansApi } from '../api'

const scans = ref<any[]>([])
const loading = ref(true)

onMounted(async () => {
  try {
    const res = await scansApi.list()
    if (res.ok) scans.value = res.data
  } finally {
    loading.value = false
  }
})

function statusLabel(s: string) {
  const m: Record<string, string> = { pending: '等待中', running: '运行中', completed: '已完成', failed: '失败' }
  return m[s] || s
}

function statusBadge(s: string) {
  const m: Record<string, string> = { running: 'badge-confirmed', completed: 'badge-clean', failed: 'badge-detected' }
  return m[s] || 'badge-unknown'
}

function formatTime(t: string) {
  if (!t) return '-'
  return new Date(t).toLocaleString('zh-CN')
}

function duration(scan: any) {
  if (!scan.started_at || !scan.completed_at) return '-'
  const diff = Math.floor((new Date(scan.completed_at).getTime() - new Date(scan.started_at).getTime()) / 1000)
  if (diff < 60) return `${diff}秒`
  return `${Math.floor(diff/60)}分${diff%60}秒`
}
</script>

<template>
  <div>
    <div class="page-header">
      <h1 class="page-title">扫描记录</h1>
      <p class="page-subtitle">查看历史扫描任务</p>
    </div>

    <div class="card">
      <div v-if="loading" style="text-align: center; padding: 40px;">
        <div class="spinner" style="margin: 0 auto;"></div>
      </div>
      <div v-else-if="scans.length === 0" class="empty-state">
        <p>暂无扫描记录</p>
      </div>
      <div v-else class="table-container">
        <table>
          <thead>
            <tr>
              <th>ID</th>
              <th>主机</th>
              <th>状态</th>
              <th>VM 总数</th>
              <th>GA</th>
              <th>磁盘</th>
              <th>命中</th>
              <th>耗时</th>
              <th>时间</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="s in scans" :key="s.id">
              <td style="font-weight: 600;">#{{ s.id }}</td>
              <td>{{ s.host_id ? `主机 #${s.host_id}` : '全部' }}</td>
              <td><span :class="['badge', statusBadge(s.status)]">{{ statusLabel(s.status) }}</span></td>
              <td>{{ s.total_vms }}</td>
              <td>{{ s.ga_count }}</td>
              <td>{{ s.disk_count }}</td>
              <td>
                <span :style="{ color: s.found_count > 0 ? 'var(--red)' : 'var(--green)', fontWeight: 600 }">
                  {{ s.found_count }}
                </span>
              </td>
              <td>{{ duration(s) }}</td>
              <td style="font-size: 13px; color: var(--text-secondary);">{{ formatTime(s.created_at) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
