<template>
  <section class="page-grid">
    <InfoCard
      title="项目工作台"
      subtitle="导入 DOCX 或文本型 PDF，系统会生成标准化文档模型并开始后续校对流程。"
    >
      <div class="hero-panel">
        <div>
          <p class="eyebrow">MVP 准备中</p>
          <h1 class="hero-title">从原始文稿到可追踪问题视图</h1>
          <p class="hero-copy">
            当前阶段先搭建工程骨架、统一布局和基础数据结构，后续会接入导入、标准化、调度与高亮链路。
          </p>
          <div class="hero-actions">
            <button
              class="primary-button"
              :disabled="loading"
              @click="handleImport"
            >
              {{ loading ? "导入中..." : "导入 DOCX" }}
            </button>
            <span class="inline-note">
              当前已接入真实导入链路，首版只支持 `.docx`
            </span>
          </div>
        </div>
        <div class="metric-group">
          <article class="metric-tile">
            <span>项目数</span>
            <strong>{{ projects.length }}</strong>
          </article>
          <article class="metric-tile">
            <span>进行中</span>
            <strong>{{ processingCount }}</strong>
          </article>
          <article class="metric-tile">
            <span>问题数</span>
            <strong>0</strong>
          </article>
        </div>
      </div>
    </InfoCard>

    <div class="two-column">
      <InfoCard
        title="项目列表"
        subtitle="这里展示本地项目记录，点击后可进入详情页。"
      >
        <div
          v-if="errorMessage"
          class="error-banner"
        >
          {{ errorMessage }}
        </div>

        <div
          v-if="projects.length"
          class="project-list"
        >
          <RouterLink
            v-for="project in projects"
            :key="project.id"
            class="project-card"
            :to="`/project/${project.id}`"
          >
            <div class="project-card__header">
              <strong>{{ project.name }}</strong>
              <span class="project-pill">{{ project.sourceType }}</span>
            </div>
            <p class="project-card__meta">{{ project.sourceFileName }}</p>
            <div class="project-card__footer">
              <span>{{ project.totalBlocks }} 个段落</span>
              <span>{{ project.status }}</span>
            </div>
          </RouterLink>
        </div>

        <div
          v-else
          class="empty-state"
        >
          <strong>还没有项目</strong>
          <p>从本地选择一个 DOCX 文件，系统会自动复制原文件、解析段落并落库。</p>
        </div>
      </InfoCard>

      <InfoCard
        title="产品原则"
        subtitle="原文件与标准化文档双存储，block 级问题锚定。"
      >
        <ul class="feature-list">
          <li>校对视图基于内部标准模型</li>
          <li>原文对照视图只做展示参考</li>
          <li>所有业务数据落本地 SQLite</li>
          <li>第一版不支持 OCR 与扫描型 PDF</li>
        </ul>
      </InfoCard>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import InfoCard from "@/components/common/InfoCard.vue";
import { importDocument, listProjects } from "@/api/projects";
import type { ProjectSummary } from "@/types/models";
import { isTauriApp } from "@/utils/runtime";

const loading = ref(false);
const errorMessage = ref("");
const projects = ref<ProjectSummary[]>([]);

const processingCount = computed(() =>
  projects.value.filter((item) => item.status === "processing").length,
);

onMounted(() => {
  void refreshProjects();
});

async function refreshProjects() {
  if (!isTauriApp()) {
    return;
  }

  projects.value = await listProjects();
}

async function handleImport() {
  if (!isTauriApp()) {
    errorMessage.value = "请通过 Tauri 桌面环境运行当前应用。";
    return;
  }

  const selected = await open({
    multiple: false,
    filters: [{ name: "Document", extensions: ["docx"] }],
  });

  if (!selected || Array.isArray(selected)) {
    return;
  }

  loading.value = true;
  errorMessage.value = "";

  try {
    await importDocument(selected);
    await refreshProjects();
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

  return "导入失败，请稍后重试。";
}
</script>
