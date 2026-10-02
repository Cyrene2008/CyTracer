<script>
import { ref } from 'vue'

let toastAdd = null
let seq = 0

export function pushToast ({ title, message, body, seconds = 4, severity = 'informational' }) {
  if (!toastAdd) return
  toastAdd({
    id: `toast-${++seq}`,
    title: title ?? '',
    message: message ?? body ?? '',
    severity,
    duration: seconds * 1000
  })
}
</script>

<script setup>
import { onMounted, ref as vueRef } from 'vue'

const toastRef = vueRef(null)
onMounted(() => {
  toastAdd = toastRef.value?.add ?? null
})
</script>

<template>
  <FluentToast ref="toastRef" class="toast-host" />
</template>

<style scoped>
.toast-host {
  position: fixed;
  right: 16px;
  bottom: 16px;
  z-index: 9000;
  width: min(360px, 80vw);
  pointer-events: none;
}
.toast-host :deep(*) {
  pointer-events: auto;
}
</style>
