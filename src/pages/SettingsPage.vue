<template>
  <section class="page-grid">
    <InfoCard
      title="模型设置"
      subtitle="这里保存 OpenAI-compatible 接口配置、并发数、超时和系统提示词模板。"
    >
      <div
        v-if="message"
        class="success-banner"
      >
        {{ message }}
      </div>
      <div
        v-if="errorMessage"
        class="error-banner"
      >
        {{ errorMessage }}
      </div>

      <div class="settings-grid">
        <label class="field">
          <span>Base URL</span>
          <input
            v-model="form.baseUrl"
            type="text"
            placeholder="https://api.openai.com/v1"
          />
        </label>
        <label class="field">
          <span>Model</span>
          <input
            v-model="form.model"
            type="text"
            placeholder="gpt-4.1-mini"
          />
        </label>
        <label class="field">
          <span>API Key</span>
          <input
            v-model="form.apiKey"
            type="password"
            placeholder="sk-..."
          />
        </label>
        <label class="field">
          <span>Timeout (ms)</span>
          <input
            v-model.number="form.timeoutMs"
            type="number"
            min="1000"
          />
        </label>
        <label class="field">
          <span>Max Concurrency</span>
          <input
            v-model.number="form.maxConcurrency"
            type="number"
            min="1"
          />
        </label>
        <label class="field">
          <span>Temperature</span>
          <input
            v-model.number="form.temperature"
            type="number"
            min="0"
            max="2"
            step="0.1"
          />
        </label>
        <label class="field">
          <span>Max Tokens</span>
          <input
            v-model.number="form.maxTokens"
            type="number"
            min="1"
          />
        </label>
      </div>

      <label class="field">
        <span>System Prompt Template</span>
        <textarea
          v-model="form.systemPromptTemplate"
          class="field__textarea"
          rows="8"
        ></textarea>
      </label>

      <div class="hero-actions">
        <button
          class="primary-button"
          :disabled="loading"
          @click="handleSave"
        >
          {{ loading ? "保存中..." : "保存设置" }}
        </button>
        <span class="inline-note">配置会保存到本地 SQLite，不会自动上传。</span>
      </div>
    </InfoCard>
  </section>
</template>

<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import InfoCard from "@/components/common/InfoCard.vue";
import { getAppSettings, saveAppSettings } from "@/api/settings";
import type { AppSettings } from "@/types/models";
import { isTauriApp } from "@/utils/runtime";

const loading = ref(false);
const message = ref("");
const errorMessage = ref("");
const form = reactive<AppSettings>({
  baseUrl: "",
  apiKey: "",
  model: "",
  timeoutMs: 60000,
  maxConcurrency: 4,
  temperature: 0.2,
  maxTokens: 1200,
  systemPromptTemplate: "",
});

onMounted(() => {
  void loadSettings();
});

async function loadSettings() {
  if (!isTauriApp()) {
    return;
  }

  loading.value = true;
  errorMessage.value = "";

  try {
    Object.assign(form, await getAppSettings());
  } catch (error) {
    errorMessage.value = extractMessage(error, "设置加载失败。");
  } finally {
    loading.value = false;
  }
}

async function handleSave() {
  if (!isTauriApp()) {
    errorMessage.value = "请通过 Tauri 桌面环境保存设置。";
    return;
  }

  loading.value = true;
  errorMessage.value = "";
  message.value = "";

  try {
    const saved = await saveAppSettings({ ...form });
    Object.assign(form, saved);
    message.value = "设置已保存。";
  } catch (error) {
    errorMessage.value = extractMessage(error, "设置保存失败。");
  } finally {
    loading.value = false;
  }
}

function extractMessage(error: unknown, fallback: string) {
  if (typeof error === "string") {
    return error;
  }

  if (error && typeof error === "object" && "message" in error) {
    return String(error.message);
  }

  return fallback;
}
</script>
