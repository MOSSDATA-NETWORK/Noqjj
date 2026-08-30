<script setup lang="ts">
import { useRoute, useRouter } from 'vue-router'
import { computed, ref, onMounted } from 'vue'
import { versionApi } from './api'

const route = useRoute()
const router = useRouter()
const currentRoute = computed(() => route.name)
const appVersion = ref('')
const updateAvailable = ref(false)
const latestVersion = ref('')

const navItems = [
  { name: 'dashboard', label: '总览', path: '/' },
  { name: 'hosts', label: '主机管理', path: '/hosts' },
  { name: 'scans', label: '扫描记录', path: '/scans' },
  { name: 'results', label: '检测结果', path: '/results' },
  { name: 'settings', label: '通知设置', path: '/settings' },
]

onMounted(async () => {
  try {
    const res = await versionApi.current()
    if (res.ok) appVersion.value = res.version
  } catch {}

  // 后台检查更新
  try {
    const res = await versionApi.check()
    if (res.ok && res.data?.update_available) {
      updateAvailable.value = true
      latestVersion.value = res.data.latest
    }
  } catch {}
})

function navigate(path: string) {
  router.push(path)
}

function goToSettings() {
  router.push('/settings')
}
</script>

<template>
  <div class="app-layout">
    <aside class="sidebar">
      <div class="sidebar-logo">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
          <path d="M9 12l2 2 4-4"/>
        </svg>
        Noqjj
      </div>
      <nav class="sidebar-nav">
        <div
          v-for="item in navItems"
          :key="item.name"
          class="nav-item"
          :class="{ active: currentRoute === item.name }"
          @click="navigate(item.path)"
        >
          <svg v-if="item.name === 'dashboard'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>
          <svg v-else-if="item.name === 'hosts'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/><line x1="6" y1="6" x2="6.01" y2="6"/><line x1="6" y1="18" x2="6.01" y2="18"/></svg>
          <svg v-else-if="item.name === 'scans'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="M21 21l-4.35-4.35"/></svg>
          <svg v-else-if="item.name === 'results'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
          <svg v-else-if="item.name === 'settings'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/></svg>
          {{ item.label }}
          <span v-if="item.name === 'settings' && updateAvailable" class="update-dot"></span>
        </div>
      </nav>
      <div class="sidebar-footer">
        <div v-if="updateAvailable" class="update-banner" @click="goToSettings">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="14" height="14"><path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
          新版本 v{{ latestVersion }}
        </div>
        <div class="version-text">v{{ appVersion || '...' }}</div>
      </div>
    </aside>
    <main class="main-content">
      <router-view />
    </main>
  </div>
</template>

<style scoped>
.sidebar-footer {
  padding: 12px;
}

.update-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: rgba(0,122,255,0.1);
  border-radius: 8px;
  font-size: 12px;
  font-weight: 600;
  color: var(--accent);
  cursor: pointer;
  margin-bottom: 8px;
  transition: background 0.2s;
}

.update-banner:hover {
  background: rgba(0,122,255,0.15);
}

.update-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--red);
  margin-left: auto;
  animation: pulse 2s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.version-text {
  font-size: 12px;
  color: var(--text-tertiary);
}
</style>
