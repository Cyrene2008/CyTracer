import { createApp } from 'vue'
import { createRouter, createWebHashHistory } from 'vue-router'
import VueFluentWidgets from 'vue-fluent-widgets'
import { addCollection } from '@iconify/vue'
import fluentIcons from '@iconify-json/fluent/icons.json'
import 'vue-fluent-widgets/style.css'
import './assets/css/main.css'
import App from './App.vue'
import { settings, loadSettingsFromBackend, onSettingsChange } from './stores/settings'

addCollection(fluentIcons)

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', redirect: '/analyze' },
    { path: '/analyze', name: 'analyze', component: () => import('./views/AnalyzeView.vue') },
    { path: '/settings', name: 'settings', component: () => import('./views/SettingsView.vue') },
    { path: '/about', name: 'about', component: () => import('./views/AboutView.vue') },
    { path: '/:pathMatch(.*)*', redirect: '/analyze' }
  ]
})

function applyTheme () {
  const root = document.documentElement
  root.classList.toggle('dark', settings.theme === 'dark')
  root.classList.toggle('light', settings.theme !== 'dark')
  root.classList.toggle('peach', settings.palette === 'peach')
}

onSettingsChange(applyTheme)
applyTheme()

const app = createApp(App)
app.use(VueFluentWidgets)
app.use(router)
app.config.errorHandler = (err) => {
  console.error('[CyTracer]', err)
}
app.mount('#app')

loadSettingsFromBackend().then(applyTheme)

export { router }
