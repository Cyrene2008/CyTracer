<script setup>
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { t } from '../../utils/i18n'

const route = useRoute()
const router = useRouter()

const items = computed(() => [
  { to: '/analyze', icon: 'fluent:video-clip-multiple-24-regular', label: t('nav.analyze') },
  { to: '/settings', icon: 'fluent:settings-24-regular', label: t('nav.settings') },
  { to: '/about', icon: 'fluent:info-24-regular', label: t('nav.about') }
])

const isActive = (item) => route.path === item.to || route.path.startsWith(item.to + '/')
</script>

<template>
  <nav class="dock">
    <button
      v-for="item in items"
      :key="item.to"
      class="dock-item"
      :class="{ active: isActive(item) }"
      :title="item.label"
      @click="router.push(item.to)"
    >
      <FluentIcon :icon="item.icon" width="20" height="20" />
      <span class="dock-label">{{ item.label }}</span>
    </button>
  </nav>
</template>

<style scoped>
.dock {
  width: var(--dock-width, 76px);
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 8px;
  border-right: 1px solid var(--border-subtle, rgba(0, 0, 0, 0.06));
  background: var(--bg-mica, rgba(255, 255, 255, 0.4));
}
.dock-item {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 10px 4px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text-secondary, #555);
  cursor: pointer;
  transition: background var(--duration-fast, 0.15s) var(--ease-standard, ease);
}
.dock-item:hover {
  background: var(--bg-hover, rgba(0, 0, 0, 0.05));
}
.dock-item.active {
  color: var(--accent, #ea5ec1);
  background: var(--accent-50, rgba(234, 94, 193, 0.08));
}
.dock-item.active::before {
  content: '';
  position: absolute;
  left: -8px;
  top: 50%;
  transform: translateY(-50%);
  width: 3px;
  height: 18px;
  border-radius: 2px;
  background: var(--accent, #ea5ec1);
}
.dock-label {
  font-size: 11px;
  line-height: 1;
}
</style>
