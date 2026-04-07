<template>
  <InfoCard
    title="原文对照"
    subtitle="使用 docx-preview 渲染原始 DOCX，仅用于查看，不参与校对。"
  >
    <div
      v-if="loading"
      class="empty-state"
    >
      <strong>正在加载原文...</strong>
      <p>首次渲染 DOCX 可能稍慢，请稍候。</p>
    </div>
    <div
      v-else-if="errorMessage"
      class="error-banner"
    >
      {{ errorMessage }}
    </div>
    <div
      v-else
      ref="containerRef"
      class="docx-preview"
    ></div>
  </InfoCard>
</template>

<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import { renderAsync } from "docx-preview";
import { readFile } from "@tauri-apps/plugin-fs";
import InfoCard from "@/components/common/InfoCard.vue";
import { isTauriApp } from "@/utils/runtime";

const props = defineProps<{
  filePath: string;
}>();

const containerRef = ref<HTMLDivElement | null>(null);
const loading = ref(false);
const errorMessage = ref("");

onMounted(() => {
  void loadPreview();
});

watch(
  () => props.filePath,
  () => {
    void loadPreview();
  },
);

async function loadPreview() {
  if (!props.filePath || !containerRef.value) {
    return;
  }

  if (!isTauriApp()) {
    errorMessage.value = "请通过 Tauri 桌面环境查看原文对照。";
    return;
  }

  loading.value = true;
  errorMessage.value = "";

  try {
    const binary = await readFile(props.filePath);
    await nextTick();
    containerRef.value!.innerHTML = "";
    await renderAsync(binary.buffer, containerRef.value!);
  } catch (error) {
    errorMessage.value = extractMessage(error);
  } finally {
    loading.value = false;
  }
}

function extractMessage(error: unknown) {
  if (typeof error === "string") {
    return error;
  }

  if (error && typeof error === "object" && "message" in error) {
    return String(error.message);
  }

  return "原文加载失败，请稍后重试。";
}
</script>
