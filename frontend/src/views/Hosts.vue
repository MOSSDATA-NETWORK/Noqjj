<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { hostsApi, scansApi } from '../api'

const hosts = ref<any[]>([])
const loading = ref(true)
const showModal = ref(false)
const editHost = ref<any>(null)
const form = ref({ name: '', host: '', port: 22, username: 'root', auth_type: 'password', password: '', ssh_key_path: '', api_token: '' })
const testing = ref<number | null>(null)
const scanning = ref(false)
const deploying = ref<number | null>(null)

onMounted(loadHosts)

async function loadHosts() {
  loading.value = true
  try {
    const res = await hostsApi.list()
    if (res.ok) hosts.value = res.data
  } finally {
    loading.value = false
  }
}

function openAdd() {
  editHost.value = null
  form.value = { name: '', host: '', port: 22, username: 'root', auth_type: 'password', password: '', ssh_key_path: '', api_token: '' }
  showModal.value = true
}

function openEdit(h: any) {
  editHost.value = h
  form.value = { name: h.name, host: h.host, port: h.port, username: h.username, auth_type: h.auth_type || 'password', password: '', ssh_key_path: h.ssh_key_path || '', api_token: '' }
  showModal.value = true
}

async function saveHost() {
  if (!form.value.name || !form.value.host) return
  const data: any = { ...form.value }
  if (!data.password) delete data.password
  if (!data.ssh_key_path) delete data.ssh_key_path
  if (!data.api_token) delete data.api_token

  if (editHost.value) {
    await hostsApi.update(editHost.value.id, data)
  } else {
    await hostsApi.create(data)
  }
  showModal.value = false
  loadHosts()
}

async function deleteHost(id: number) {
  if (!confirm('确定删除此主机？')) return
  await hostsApi.delete(id)
  loadHosts()
}

async function testHost(id: number) {
  testing.value = id
  try {
    const res = await hostsApi.test(id)
    alert(res.message || res.error || '测试完成')
    loadHosts()
  } finally {
    testing.value = null
  }
}

async function deployAgent(id: number) {
  deploying.value = id
  try {
    const res = await hostsApi.deploy(id)
    alert(res.message || res.error)
    loadHosts()
  } finally {
    deploying.value = null
  }
}

async function scanAll() {
  scanning.value = true
  try { await scansApi.create(); alert('扫描已启动') } finally { scanning.value = false }
}

async function scanHost(hostId: number) {
  scanning.value = true
  try { await scansApi.create(hostId); alert('扫描已启动') } finally { scanning.value = false }
}

function authTypeLabel(t: string) {
  const m: Record<string, string> = { password: '密码', ssh_key: 'SSH Key', api_token: 'API Token' }
  return m[t] || t
}
</script>

<template>
  <div>
    <div class="page-header" style="display: flex; justify-content: space-between; align-items: flex-start;">
      <div>
        <h1 class="page-title">主机管理</h1>
        <p class="page-subtitle">管理 PVE 宿主机，添加后自动部署检测脚本</p>
      </div>
      <div style="display: flex; gap: 12px;">
        <button class="btn btn-secondary" @click="scanAll" :disabled="scanning">{{ scanning ? '扫描中...' : '扫描全部' }}</button>
        <button class="btn btn-primary" @click="openAdd">+ 添加主机</button>
      </div>
    </div>

    <div class="card">
      <div v-if="loading" style="text-align: center; padding: 40px;"><div class="spinner" style="margin: 0 auto;"></div></div>
      <div v-else-if="hosts.length === 0" class="empty-state">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" width="48" height="48"><rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/><line x1="6" y1="6" x2="6.01" y2="6"/><line x1="6" y1="18" x2="6.01" y2="18"/></svg>
        <p>还没有添加主机</p>
        <button class="btn btn-primary" @click="openAdd">添加第一台 PVE</button>
      </div>
      <div v-else class="table-container">
        <table>
          <thead>
            <tr><th>名称</th><th>地址</th><th>接入方式</th><th>状态</th><th>Agent</th><th style="text-align: right;">操作</th></tr>
          </thead>
          <tbody>
            <tr v-for="h in hosts" :key="h.id">
              <td style="font-weight: 600;">{{ h.name }}</td>
              <td><code style="font-size: 13px;">{{ h.host }}:{{ h.port }}</code></td>
              <td>{{ authTypeLabel(h.auth_type) }}</td>
              <td><span :class="['badge', `badge-${h.status}`]">{{ h.status }}</span></td>
              <td>
                <span v-if="h.agent_deployed" class="badge badge-online">已部署</span>
                <span v-else class="badge badge-unknown">未部署</span>
              </td>
              <td style="text-align: right;">
                <div style="display: flex; gap: 8px; justify-content: flex-end;">
                  <button class="btn btn-sm btn-secondary" @click="scanHost(h.id)" :disabled="scanning">扫描</button>
                  <button class="btn btn-sm btn-secondary" @click="testHost(h.id)" :disabled="testing === h.id">
                    {{ testing === h.id ? '测试中...' : '测试连接' }}
                  </button>
                  <button v-if="!h.agent_deployed" class="btn btn-sm btn-secondary" @click="deployAgent(h.id)" :disabled="deploying === h.id">
                    {{ deploying === h.id ? '部署中...' : '部署Agent' }}
                  </button>
                  <button class="btn btn-sm btn-secondary" @click="openEdit(h)">编辑</button>
                  <button class="btn btn-sm btn-danger" @click="deleteHost(h.id)">删除</button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Add/Edit Modal -->
    <div v-if="showModal" class="modal-overlay" @click.self="showModal = false">
      <div class="modal">
        <div class="modal-header">{{ editHost ? '编辑主机' : '添加主机' }}</div>
        <div class="modal-body">
          <div class="form-group">
            <label class="form-label">名称</label>
            <input class="form-input" v-model="form.name" placeholder="如：tpe2-srv18" />
          </div>
          <div class="form-group">
            <label class="form-label">IP 地址</label>
            <input class="form-input" v-model="form.host" placeholder="78.105.182.253" />
          </div>

          <div class="form-group">
            <label class="form-label">接入方式</label>
            <select class="form-input" v-model="form.auth_type">
              <option value="password">SSH 密码</option>
              <option value="ssh_key">SSH 私钥</option>
              <option value="api_token">PVE API Token</option>
            </select>
          </div>

          <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px;">
            <div class="form-group">
              <label class="form-label">SSH 端口</label>
              <input class="form-input" type="number" v-model.number="form.port" />
            </div>
            <div class="form-group">
              <label class="form-label">用户名</label>
              <input class="form-input" v-model="form.username" />
            </div>
          </div>

          <div v-if="form.auth_type === 'password'" class="form-group">
            <label class="form-label">密码 {{ editHost ? '(留空不修改)' : '' }}</label>
            <input class="form-input" type="password" v-model="form.password" placeholder="SSH 密码" />
          </div>

          <div v-if="form.auth_type === 'ssh_key'" class="form-group">
            <label class="form-label">SSH 私钥路径</label>
            <input class="form-input" v-model="form.ssh_key_path" placeholder="/root/.ssh/id_rsa" />
          </div>

          <div v-if="form.auth_type === 'api_token'" class="form-group">
            <label class="form-label">PVE API Token</label>
            <input class="form-input" v-model="form.api_token" placeholder="user@pve!tokenid=secret" />
            <div style="font-size: 12px; color: var(--text-secondary); margin-top: 4px;">
              格式：用户名@pve!token名=token值，需要在 PVE 数据中心 → API Token 中创建
            </div>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-secondary" @click="showModal = false">取消</button>
          <button class="btn btn-primary" @click="saveHost">保存</button>
        </div>
      </div>
    </div>
  </div>
</template>
