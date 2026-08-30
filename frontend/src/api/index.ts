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
