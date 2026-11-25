import axios from 'axios'

const api = axios.create({ baseURL: '/api' })

export const hostsApi = {
  list: () => api.get('/hosts').then(r => r.data),
  get: (id: number) => api.get(`/hosts/${id}`).then(r => r.data),
  create: (data: any) => api.post('/hosts', data).then(r => r.data),
  update: (id: number, data: any) => api.put(`/hosts/${id}`, data).then(r => r.data),
  delete: (id: number) => api.delete(`/hosts/${id}`).then(r => r.data),
  test: (id: number) => api.post(`/hosts/${id}/test`).then(r => r.data),
  deploy: (id: number) => api.post(`/hosts/${id}/deploy`).then(r => r.data),
}

export const scansApi = {
  list: (limit = 20, offset = 0) => api.get('/scans', { params: { limit, offset } }).then(r => r.data),
  get: (id: number) => api.get(`/scans/${id}`).then(r => r.data),
  create: (hostId?: number) => api.post('/scans', { host_id: hostId }).then(r => r.data),
  stop: (id: number) => api.post(`/scans/${id}/stop`).then(r => r.data),
}

export const resultsApi = {
  list: (params?: { host_id?: number; status?: string; limit?: number; offset?: number }) =>
    api.get('/results', { params }).then(r => r.data),
  stats: () => api.get('/results/stats').then(r => r.data),
  history: (id: number) => api.get(`/results/${id}/history`).then(r => r.data),
}

export const schedulesApi = {
  list: () => api.get('/schedules').then(r => r.data),
  create: (data: { host_id?: number | null; cron_expr: string; enabled?: boolean; auto_disk_scan?: boolean }) =>
    api.post('/schedules', data).then(r => r.data),
  update: (id: number, data: { cron_expr?: string; enabled?: boolean; auto_disk_scan?: boolean }) =>
    api.put(`/schedules/${id}`, data).then(r => r.data),
  delete: (id: number) => api.delete(`/schedules/${id}`).then(r => r.data),
  run: (id: number) => api.post(`/schedules/${id}/run`).then(r => r.data),
}

// 磁盘扫描：后端队列执行，网页切走不影响
export const diskScanApi = {
  scanVm: (hostId: number, vmid: string) => api.post(`/hosts/${hostId}/scan-vm`, { vmid }).then(r => r.data),
  batch: () => api.post('/disk-scans').then(r => r.data),
  active: () => api.get('/disk-scans/active').then(r => r.data),
  get: (id: number) => api.get(`/disk-scans/${id}`).then(r => r.data),
  stop: (id: number) => api.post(`/disk-scans/${id}/stop`).then(r => r.data),
  whitelist: () => api.get('/disk-whitelist').then(r => r.data),
  whitelistAdd: (hostId: number, vmid: string) => api.post('/disk-whitelist', { host_id: hostId, vmid }).then(r => r.data),
  whitelistRemove: (hostId: number, vmid: string) => api.delete(`/disk-whitelist/${hostId}/${vmid}`).then(r => r.data),
  settings: () => api.get('/disk-settings').then(r => r.data),
  saveSettings: (data: { clean_count: number; days: number }) => api.put('/disk-settings', data).then(r => r.data),
}

export const notificationsApi = {
  list: () => api.get('/notifications').then(r => r.data),
  create: (data: any) => api.post('/notifications', data).then(r => r.data),
  update: (id: number, data: any) => api.put(`/notifications/${id}`, data).then(r => r.data),
  test: () => api.post('/notifications/test').then(r => r.data),
}

export const versionApi = {
  current: () => api.get('/version').then(r => r.data),
  check: () => api.get('/version/check').then(r => r.data),
  changelog: () => api.get('/version/changelog').then(r => r.data),
  update: () => api.post('/version/update').then(r => r.data),
}

export const authApi = {
  changePassword: (data: { old_password: string; new_password: string }) => api.post('/auth/password', data).then(r => r.data),
  resetTotp: (password: string) => api.post('/auth/reset-totp', { password }).then(r => r.data),
  disableTotp: (password: string) => api.post('/auth/disable-totp', { password }).then(r => r.data),
}

export const passkeyApi = {
  hasPasskey: (username?: string) => api.post('/passkey/has', username ? { username } : {}).then(r => r.data),
  loginStart: (username?: string) => api.post('/passkey/login/start', username ? { username } : {}).then(r => r.data),
  loginFinish: (username: string, credential: any) => api.post('/passkey/login/finish', { username: username || undefined, credential }).then(r => r.data),
  registerStart: () => api.post('/passkey/register/start').then(r => r.data),
  registerFinish: (credential: any) => api.post('/passkey/register/finish', credential).then(r => r.data),
  delete: () => api.post('/passkey/delete').then(r => r.data),
}
