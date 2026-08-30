<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import axios from 'axios'

const router = useRouter()
const step = ref(1) // 1=基础设置, 2=TOTP设置, 3=完成
const username = ref('admin')
const password = ref('')
const confirmPassword = ref('')
const enableTotp = ref(false)
const loading = ref(false)
const error = ref('')
const totpSecret = ref('')
const totpUri = ref('')

onMounted(async () => {
  try {
    const res = await axios.get('/api/auth/check')
    if (res.data.initialized) {
      router.push('/login')
    }
  } catch {}
})

async function doSetup() {
  if (password.value.length < 8) {
    error.value = '密码至少8位'
    return
  }
  if (password.value !== confirmPassword.value) {
    error.value = '两次密码不一致'
    return
  }

  loading.value = true
  error.value = ''
  try {
    const res = await axios.post('/api/auth/setup', {
      username: username.value,
      password: password.value,
      enable_totp: enableTotp.value,
    })
    if (res.data.ok) {
      if (res.data.totp_secret) {
        totpSecret.value = res.data.totp_secret
        totpUri.value = res.data.totp_uri
        step.value = 2
      } else {
        step.value = 3
      }
    } else {
      error.value = res.data.error
    }
  } catch (e: any) {
    error.value = e.response?.data?.error || '设置失败'
  } finally {
    loading.value = false
  }
}

function finish() {
  router.push('/')
}
</script>

<template>
  <div class="setup-page">
    <div class="setup-card">
      <!-- Step 1: 基础设置 -->
      <template v-if="step === 1">
        <div class="setup-logo">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="40" height="40">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
            <path d="M9 12l2 2 4-4"/>
          </svg>
        </div>
        <h1 class="setup-title">初始化设置</h1>
        <p class="setup-subtitle">首次运行，请设置管理员账户</p>

        <div class="form-group">
          <label class="form-label">用户名</label>
          <input class="form-input" v-model="username" placeholder="admin" />
        </div>
        <div class="form-group">
          <label class="form-label">密码</label>
          <input class="form-input" type="password" v-model="password" placeholder="至少8位" />
        </div>
        <div class="form-group">
          <label class="form-label">确认密码</label>
          <input class="form-input" type="password" v-model="confirmPassword" placeholder="再次输入密码" />
        </div>

        <div class="form-group" style="display: flex; align-items: center; gap: 10px;">
          <input type="checkbox" id="totp" v-model="enableTotp" style="width: 18px; height: 18px;" />
          <label for="totp" style="font-size: 14px; cursor: pointer;">启用 TOTP 身份验证器（推荐）</label>
        </div>

        <div v-if="error" style="color: var(--red); font-size: 14px; margin-bottom: 16px;">{{ error }}</div>

        <button class="btn btn-primary" style="width: 100%;" @click="doSetup" :disabled="loading">
          {{ loading ? '设置中...' : '完成设置' }}
        </button>
      </template>

      <!-- Step 2: TOTP 二维码 -->
      <template v-if="step === 2">
        <h1 class="setup-title">绑定身份验证器</h1>
        <p class="setup-subtitle">使用 Google Authenticator / Authy 等 App 扫描二维码</p>

        <div style="text-align: center; margin: 24px 0;">
          <div style="background: white; display: inline-block; padding: 16px; border-radius: 12px;">
            <img :src="'https://api.qrserver.com/v1/create-qr-code/?size=200x200&data=' + encodeURIComponent(totpUri)" alt="TOTP QR Code" width="200" height="200" />
          </div>
        </div>

        <div class="form-group">
          <label class="form-label">密钥（手动输入）</label>
          <input class="form-input" :value="totpSecret" readonly style="font-family: monospace; text-align: center;" />
        </div>

        <button class="btn btn-primary" style="width: 100%;" @click="step = 3">
          已绑定，继续
        </button>
      </template>

      <!-- Step 3: 完成 -->
      <template v-if="step === 3">
        <div style="text-align: center;">
          <div style="font-size: 48px; margin-bottom: 16px;">✅</div>
          <h1 class="setup-title">设置完成</h1>
          <p class="setup-subtitle">管理员账户已创建，开始使用吧</p>
          <button class="btn btn-primary" style="margin-top: 24px;" @click="finish">
            进入控制台
          </button>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.setup-page {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg);
}
.setup-card {
  background: var(--bg-secondary);
  border-radius: 20px;
  box-shadow: var(--shadow-lg);
  padding: 40px;
  width: 100%;
  max-width: 420px;
}
.setup-logo { text-align: center; color: var(--accent); margin-bottom: 16px; }
.setup-title { text-align: center; font-size: 24px; font-weight: 700; margin-bottom: 4px; }
.setup-subtitle { text-align: center; font-size: 14px; color: var(--text-secondary); margin-bottom: 28px; }
</style>
