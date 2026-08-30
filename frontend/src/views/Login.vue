<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import axios from 'axios'

const router = useRouter()
const username = ref('')
const password = ref('')
const totpCode = ref('')
const loading = ref(false)
const error = ref('')
const needsMfa = ref(false)

onMounted(async () => {
  try {
    const res = await axios.get('/api/auth/check')
    if (!res.data.initialized) {
      router.push('/setup')
    }
  } catch {}
})

async function doLogin() {
  if (!username.value || !password.value) {
    error.value = '请输入用户名和密码'
    return
  }
  loading.value = true
  error.value = ''
  try {
    const res = await axios.post('/api/auth/login', {
      username: username.value,
      password: password.value,
    })
    if (res.data.ok) {
      if (res.data.needs_mfa) {
        needsMfa.value = true
      } else {
        router.push('/')
      }
    } else {
      error.value = res.data.error || '登录失败'
    }
  } catch (e: any) {
    error.value = e.response?.data?.error || '连接失败'
  } finally {
    loading.value = false
  }
}

async function verifyTotp() {
  if (!totpCode.value) return
  loading.value = true
  error.value = ''
  try {
    const res = await axios.post('/api/auth/verify-totp', { code: totpCode.value })
    if (res.data.ok) {
      router.push('/')
    } else {
      error.value = res.data.error || '验证码错误'
    }
  } catch (e: any) {
    error.value = e.response?.data?.error || '验证失败'
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="login-page">
    <div class="login-card">
      <div class="login-logo">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="40" height="40">
          <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
          <path d="M9 12l2 2 4-4"/>
        </svg>
      </div>
      <h1 class="login-title">Chicken Detect</h1>
      <p class="login-subtitle">PVE 切鸡检测平台</p>

      <!-- 密码登录 -->
      <template v-if="!needsMfa">
        <div class="form-group">
          <label class="form-label">用户名</label>
          <input class="form-input" v-model="username" placeholder="admin" @keyup.enter="doLogin" />
        </div>
        <div class="form-group">
          <label class="form-label">密码</label>
          <input class="form-input" type="password" v-model="password" placeholder="请输入密码" @keyup.enter="doLogin" />
        </div>
      </template>

      <!-- TOTP 验证 -->
      <template v-else>
        <div style="text-align: center; margin-bottom: 20px;">
          <div style="font-size: 32px; margin-bottom: 8px;">🔐</div>
          <p style="font-size: 14px; color: var(--text-secondary);">请输入身份验证器中的6位验证码</p>
        </div>
        <div class="form-group">
          <input class="form-input" v-model="totpCode" placeholder="000000" maxlength="6"
            style="text-align: center; font-size: 24px; letter-spacing: 8px; font-family: monospace;"
            @keyup.enter="verifyTotp" />
        </div>
        <button class="btn btn-primary" style="width: 100%; margin-bottom: 12px;" @click="verifyTotp" :disabled="loading">
          验证
        </button>
        <button class="btn btn-secondary" style="width: 100%;" @click="needsMfa = false; totpCode = ''">
          返回
        </button>
      </template>

      <div v-if="error" style="color: var(--red); font-size: 14px; margin: 12px 0;">{{ error }}</div>

      <button v-if="!needsMfa" class="btn btn-primary" style="width: 100%;" @click="doLogin" :disabled="loading">
        {{ loading ? '登录中...' : '登录' }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.login-page {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg);
}
.login-card {
  background: var(--bg-secondary);
  border-radius: 20px;
  box-shadow: var(--shadow-lg);
  padding: 40px;
  width: 100%;
  max-width: 380px;
}
.login-logo { text-align: center; color: var(--accent); margin-bottom: 16px; }
.login-title { text-align: center; font-size: 24px; font-weight: 700; margin-bottom: 4px; }
.login-subtitle { text-align: center; font-size: 14px; color: var(--text-secondary); margin-bottom: 28px; }
</style>
