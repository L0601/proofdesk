<template>
  <section class="page-grid">
    <InfoCard
      title="项目详情"
      subtitle="这里会承接项目元信息、进度、调用日志、问题列表和正文联动。"
    >
      <div class="detail-banner">
        <div>
          <p class="eyebrow">项目</p>
          <h1 class="hero-title">{{ projectTitle }}</h1>
        </div>
        <div class="hero-actions">
          <button
            class="ghost-button"
            :class="{ 'ghost-button--active': activeView === 'proofread' }"
            @click="activeView = 'proofread'"
          >
            校对视图
          </button>
          <button
            class="ghost-button"
            :class="{ 'ghost-button--active': activeView === 'source' }"
            :disabled="!projectDetail"
            @click="activeView = 'source'"
          >
            原文对照
          </button>
          <RouterLink
            class="ghost-link"
            to="/"
          >
            返回项目列表
          </RouterLink>
        </div>
      </div>
    </InfoCard>

    <div
      v-if="loadError"
      class="error-banner"
    >
      {{ loadError }}
    </div>

    <div
      v-else-if="loading"
      class="empty-state"
    >
      <strong>正在加载项目详情...</strong>
      <p>稍后会展示项目元信息、正文和原文对照视图。</p>
    </div>

    <div
      v-else
      class="detail-layout"
    >
      <TiptapDemo v-if="activeView === 'proofread'" />
      <DocxSourcePreview
        v-else-if="projectDetail?.sourceType === 'docx'"
        :file-path="projectDetail.sourceFilePath"
      />

      <InfoCard
        title="右侧问题面板"
        subtitle="后续将在这里展示 issue 列表、详情和跳转操作。"
      >
        <div class="placeholder-stack">
          <div class="placeholder-row">
            <span class="status-dot"></span>
            <div>
              <strong>暂无问题数据</strong>
              <p>完成数据层和 AI 调度后，这里会联动正文高亮。</p>
            </div>
          </div>
          <div class="placeholder-row">
            <span class="status-dot status-dot--warn"></span>
            <div>
              <strong>当前已接入 Tiptap</strong>
              <p>用于验证编辑器依赖、只读渲染与样式表现。</p>
            </div>
          </div>
        </div>
      </InfoCard>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import InfoCard from "@/components/common/InfoCard.vue";
import TiptapDemo from "@/components/editor/TiptapDemo.vue";
import DocxSourcePreview from "@/components/preview/DocxSourcePreview.vue";
import { getProjectDetail } from "@/api/projects";
import type { ProjectDetail } from "@/types/models";
import { isTauriApp } from "@/utils/runtime";

const route = useRoute();
const activeView = ref<"proofread" | "source">("proofread");
const loading = ref(false);
const loadError = ref("");
const projectDetail = ref<ProjectDetail | null>(null);

const projectId = computed(() => String(route.params.id ?? ""));
const projectTitle = computed(() => projectDetail.value?.name ?? projectId.value);

onMounted(() => {
  void loadProjectDetail();
});

async function loadProjectDetail() {
  if (!isTauriApp()) {
    return;
  }

  loading.value = true;
  loadError.value = "";

  try {
    projectDetail.value = await getProjectDetail(projectId.value);
    if (!projectDetail.value) {
      loadError.value = "未找到对应项目记录。";
    }
  } catch (error) {
    loadError.value = extractMessage(error);
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

  return "项目详情加载失败。";
}
</script>
