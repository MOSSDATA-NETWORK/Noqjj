<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { notificationsApi, versionApi } from '../api'

const notifications = ref<any[]>([])
const loading = ref(true)
const showModal = ref(false)
const editItem = ref<any>(null)
const testing = ref(false)

// Version
const currentVersion = ref('')
const updateInfo = ref<any>(null)
const checkingUpdate = ref(false)
const updating = ref(false)
const changelog = ref<any[]>([])
const showChangelog = ref(false)

// Telegram form
const tgForm = ref({ bot_token: '', chat_id: '' })
// WeCom form
const wcForm = ref({ webhook: '' })
const notifyLevel = ref('detected_and_cleaned')

onMounted(async () => {
  loadNotifications()
  loadVersion()
})

async function loadVersion() {
  try {
    const res = await versionApi.current()
    if (res.ok) currentVersion.value = res.version
  } catch {}
}

async function checkUpdate() {
  checkingUpdate.value = true
  try {
    const res = await versionApi.check()
    if (res.ok) {
      updateInfo.value = res.data
      if (!res.data.update_available) {
        alert('当前已是最新版本')
      }
    }
  } finally {
    checkingUpdate.value = false
  }
}

async function loadChangelog() {
  try {
    const res = await versionApi.changelog()
    if (res.ok) changelog.value = res.data
  } catch {}
  showChangelog.value = true
}

async function doUpdate() {
  if (!confirm(`确定要更新到 v${updateInfo.value?.latest}？更新后需要手动重启服务。`)) return
  updating.value = true
  try {
    const res = await versionApi.update()
    alert(res.message || res.error)
  } finally {
    updating.value = false
  }
}

async function loadNotifications() {
  loading.value = true
  try {
    const res = await notificationsApi.list()
    if (res.ok) notifications.value = res.data
  } finally {
    loading.value = false
  }
}

function openAdd(_type: string) {
  editItem.value = null
  tgForm.value = { bot_token: '', chat_id: '' }
  wcForm.value = { webhook: '' }
  notifyLevel.value = 'detected_and_cleaned'
  showModal.value = true
}

function openEdit(n: any) {
  editItem.value = n
  try {
    const config = JSON.parse(n.config)
    if (n.type === 'telegram') {
      tgForm.value = { bot_token: config.bot_token || '', chat_id: config.chat_id || '' }
    } else {
      wcForm.value = { webhook: config.webhook || '' }
    }
  } catch {}
  notifyLevel.value = n.notify_level || 'detected_and_cleaned'
  showModal.value = true
}

async function saveNotification() {
  const type = editItem.value?.type || (tgForm.value.bot_token ? 'telegram' : 'wecom')
  const config = type === 'telegram' ? JSON.stringify(tgForm.value) : JSON.stringify(wcForm.value)
  const data = { type, enabled: true, config, notify_level: notifyLevel.value }
  if (editItem.value) {
    await notificationsApi.update(editItem.value.id, data)
  } else {
    await notificationsApi.create(data)
  }
  showModal.value = false
  loadNotifications()
}

async function testNotifications() {
  testing.value = true
  try {
    const res = await notificationsApi.test()
    if (res.ok) {
      const msgs = res.data.map((r: any) => `${r.type}: ${r.success ? '✅' : '❌'} ${r.message}`).join('\n')
      alert(msgs || '没有启用的通知')
    }
  } finally {
    testing.value = false
  }
}

function levelLabel(l: string) {
  const m: Record<string, string> = { all: '全部通知', detected_only: '仅新发现', detected_and_cleaned: '新发现 + 已清除' }
  return m[l] || l
}

function formatDate(d: string) {
  if (!d) return ''
  return new Date(d).toLocaleString('zh-CN', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })
}
</script>

<template>
  <div>
    <!-- Version & Update Section -->
    <div class="page-header">
      <div>
        <h1 class="page-title">设置</h1>
        <p class="page-subtitle">系统设置与更新</p>
      </div>
    </div>

    <!-- Version Card -->
    <div class="card" style="margin-bottom: 16px;">
      <div class="card-header">
        <h3>版本信息</h3>
      </div>
      <div style="display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 16px;">
        <div>
          <div style="font-size: 28px; font-weight: 700;">Noqjj <span style="color: var(--accent);">v{{ currentVersion }}</span></div>
          <div v-if="updateInfo?.update_available" style="margin-top: 8px;">
            <span class="badge badge-detected" style="font-size: 13px;">
              <span class="badge-dot"></span>
              新版本 v{{ updateInfo.latest }} 可用
            </span>
          </div>
          <div v-else-if="updateInfo && !updateInfo.update_available" style="margin-top: 8px; color: var(--green); font-size: 14px;">
            ✅ 当前已是最新版本
          </div>
        </div>
        <div style="display: flex; gap: 12px;">
          <button class="btn btn-secondary" @click="checkUpdate" :disabled="checkingUpdate">
            {{ checkingUpdate ? '检查中...' : '检查更新' }}
          </button>
          <button class="btn btn-secondary" @click="loadChangelog">更新日志</button>
          <button v-if="updateInfo?.update_available" class="btn btn-primary" @click="doUpdate" :disabled="updating">
            {{ updating ? '更新中...' : '立即更新' }}
          </button>
        </div>
      </div>

      <!-- Update Info -->
      <div v-if="updateInfo?.update_available" style="margin-top: 16px; padding: 16px; background: rgba(0,122,255,0.05); border-radius: 12px;">
        <div style="font-weight: 600; margin-bottom: 8px;">v{{ updateInfo.latest }} 更新内容</div>
        <div style="font-size: 14px; color: var(--text-secondary); white-space: pre-wrap; max-height: 200px; overflow-y: auto;">{{ updateInfo.release_notes || '暂无更新说明' }}</div>
        <a v-if="updateInfo.release_url" :href="updateInfo.release_url" target="_blank" style="display: inline-block; margin-top: 8px; font-size: 13px; color: var(--accent);">
          在 GitHub 查看 →
        </a>
      </div>
    </div>

    <!-- Changelog Modal -->
    <div v-if="showChangelog" class="modal-overlay" @click.self="showChangelog = false">
      <div class="modal" style="max-width: 600px;">
        <div class="modal-header">更新日志</div>
        <div class="modal-body" style="max-height: 60vh; overflow-y: auto;">
          <div v-if="changelog.length === 0" style="text-align: center; padding: 20px; color: var(--text-secondary);">
            暂无更新日志
          </div>
          <div v-for="(entry, i) in changelog" :key="i" class="changelog-entry">
            <div class="changelog-header">
              <span class="changelog-version">{{ entry.name }}</span>
              <span class="changelog-date">{{ formatDate(entry.published_at) }}</span>
            </div>
            <div class="changelog-notes" v-html="entry.notes || '暂无说明'"></div>
            <a :href="entry.url" target="_blank" class="changelog-link">在 GitHub 查看 →</a>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="showChangelog = false">关闭</button>
        </div>
      </div>
    </div>

    <!-- Notification Section -->
    <div style="display: flex; justify-content: space-between; align-items: center; margin: 28px 0 16px;">
      <h2 style="font-size: 20px; font-weight: 600;">通知设置</h2>
      <div style="display: flex; gap: 12px;">
        <button class="btn btn-secondary" @click="testNotifications" :disabled="testing">
          {{ testing ? '发送中...' : '测试通知' }}
        </button>
        <button class="btn btn-primary" @click="openAdd('telegram')">+ 添加通知</button>
      </div>
    </div>

    <div class="card">
      <div v-if="loading" style="text-align: center; padding: 40px;">
        <div class="spinner" style="margin: 0 auto;"></div>
      </div>
      <div v-else-if="notifications.length === 0" class="empty-state">
        <p>还没有配置通知</p>
        <div style="display: flex; gap: 12px; justify-content: center; margin-top: 16px;">
          <button class="btn btn-primary" @click="openAdd('telegram')">添加 Telegram</button>
          <button class="btn btn-primary" @click="openAdd('wecom')">添加企业微信</button>
        </div>
      </div>
      <div v-else class="table-container">
        <table>
          <thead>
            <tr><th>类型</th><th>状态</th><th>通知级别</th><th>配置</th><th style="text-align: right;">操作</th></tr>
          </thead>
          <tbody>
            <tr v-for="n in notifications" :key="n.id">
              <td style="font-weight: 600;">{{ n.type === 'telegram' ? '📱 Telegram' : '💬 企业微信' }}</td>
              <td><span :class="['badge', n.enabled ? 'badge-online' : 'badge-offline']">{{ n.enabled ? '已启用' : '已禁用' }}</span></td>
              <td>{{ levelLabel(n.notify_level) }}</td>
              <td style="font-size: 13px; color: var(--text-secondary); max-width: 300px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{{ n.config }}</td>
              <td style="text-align: right;"><button class="btn btn-sm btn-secondary" @click="openEdit(n)">编辑</button></td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Add/Edit Modal -->
    <div v-if="showModal" class="modal-overlay" @click.self="showModal = false">
      <div class="modal">
        <div class="modal-header">{{ editItem ? '编辑通知' : '添加通知' }}</div>
        <div class="modal-body">
          <template v-if="!editItem || editItem.type === 'telegram'">
            <div class="form-group">
              <label class="form-label">Telegram Bot Token</label>
              <input class="form-input" v-model="tgForm.bot_token" placeholder="123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11" />
            </div>
            <div class="form-group">
              <label class="form-label">Chat ID</label>
              <input class="form-input" v-model="tgForm.chat_id" placeholder="123456789" />
            </div>
          </template>
          <template v-if="editItem && editItem.type === 'wecom'">
            <div class="form-group">
              <label class="form-label">企业微信 Webhook URL</label>
              <input class="form-input" v-model="wcForm.webhook" placeholder="https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=..." />
            </div>
          </template>
          <div class="form-group">
            <label class="form-label">通知级别</label>
            <select class="form-input" v-model="notifyLevel">
              <option value="all">全部通知</option>
              <option value="detected_only">仅新发现</option>
              <option value="detected_and_cleaned">新发现 + 已清除</option>
            </select>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="showModal = false">取消</button>
          <button class="btn btn-primary" @click="saveNotification">保存</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.changelog-entry {
  padding: 16px 0;
  border-bottom: 1px solid var(--border);
}
.changelog-entry:last-child { border-bottom: none; }
.changelog-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}
.changelog-version {
  font-size: 16px;
  font-weight: 600;
}
.changelog-date {
  font-size: 13px;
  color: var(--text-secondary);
}
.changelog-notes {
  font-size: 14px;
  color: var(--text-secondary);
  white-space: pre-wrap;
  max-height: 150px;
  overflow-y: auto;
  margin-bottom: 8px;
}
.changelog-link {
  font-size: 13px;
  color: var(--accent);
  text-decoration: none;
}
.changelog-link:hover { text-decoration: underline; }
</style>
