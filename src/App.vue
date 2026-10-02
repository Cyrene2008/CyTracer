<script setup>
import { onMounted, onUnmounted } from 'vue'
import TitleBar from './components/layout/TitleBar.vue'
import Dock from './components/layout/Dock.vue'
import ToastHost from './components/ToastHost.vue'
import { backendState, startBackendPoll, stopBackendPoll } from './stores/backend'
import { t } from './utils/i18n'

onMounted(startBackendPoll)
onUnmounted(stopBackendPoll)
</script>

<template>
  <div class="app-shell">
    <TitleBar />
    <FluentInfoBar
      v-if="backendState.checked && !backendState.ready"
      class="backend-warning"
      severity="warning"
      :title="t('status.failed')"
      message="FFmpeg / 引擎未就绪，请检查安装完整性。"
      :closable="false"
    />
    <div class="app-body">
      <Dock />
      <div class="page-view">
        <router-view v-slot="{ Component }">
          <transition name="page-forward" mode="out-in">
            <component :is="Component" />
          </transition>
        </router-view>
      </div>
    </div>
    <ToastHost />
  </div>
</template>

<style scoped>
.backend-warning {
  margin: 8px 12px 0;
}
</style>
