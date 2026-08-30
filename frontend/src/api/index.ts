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
  list: () => api.get('/scans').then(r => r.data),
  get: (id: number) => api.get(`/scans/${id}`).then(r => r.data),
  create: (hostId?: number) => api.post('/scans', { host_id: hostId }).then(r => r.data),
}

export const resultsApi = {
  list: (hostId?: number) => api.get('/results', { params: hostId ? { host_id: hostId } : {} }).then(r => r.data),
  stats: () => api.get('/results/stats').then(r => r.data),
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
  hasPasskey: (username: string) => api.post('/passkey/has', { username }).then(r => r.data),
  loginStart: (username: string) => api.post('/passkey/login/start', { username }).then(r => r.data),
  loginFinish: (username: string, credential: any) => api.post('/passkey/login/finish', { username, credential }).then(r => r.data),
  registerStart: () => api.post('/passkey/register/start').then(r => r.data),
  registerFinish: (credential: any) => api.post('/passkey/register/finish', credential).then(r => r.data),
  delete: () => api.post('/passkey/delete').then(r => r.data),
}
