<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { notificationsApi } from '../api'

const notifications = ref<any[]>([])
const loading = ref(true)
const showModal = ref(false)
const editItem = ref<any>(null)
const testing = ref(false)

// Telegram form
const tgForm = ref({ bot_token: '', chat_id: '' })
// WeCom form
const wcForm = ref({ webhook: '' })
const notifyLevel = ref('detected_and_cleaned')

onMounted(loadNotifications)

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
  const config = type === 'telegram'
    ? JSON.stringify(tgForm.value)
    : JSON.stringify(wcForm.value)

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
  const m: Record<string, string> = {
    all: '全部通知',
    detected_only: '仅新发现',
    detected_and_cleaned: '新发现 + 已清除',
  }
  return m[l] || l
}
</script>

<template>
  <div>
    <div class="page-header" style="display: flex; justify-content: space-between; align-items: flex-start;">
      <div>
        <h1 class="page-title">通知设置</h1>
        <p class="page-subtitle">配置 Telegram 和企业微信告警</p>
      </div>
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
            <tr>
              <th>类型</th>
              <th>状态</th>
              <th>通知级别</th>
              <th>配置</th>
              <th style="text-align: right;">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="n in notifications" :key="n.id">
              <td style="font-weight: 600;">
                {{ n.type === 'telegram' ? '📱 Telegram' : '💬 企业微信' }}
              </td>
              <td>
                <span :class="['badge', n.enabled ? 'badge-online' : 'badge-offline']">
                  {{ n.enabled ? '已启用' : '已禁用' }}
                </span>
              </td>
              <td>{{ levelLabel(n.notify_level) }}</td>
              <td style="font-size: 13px; color: var(--text-secondary); max-width: 300px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                {{ n.config }}
              </td>
              <td style="text-align: right;">
                <button class="btn btn-sm btn-secondary" @click="openEdit(n)">编辑</button>
              </td>
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
          <!-- Telegram fields -->
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

          <!-- WeCom fields -->
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
