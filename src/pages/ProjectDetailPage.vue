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
            class="primary-button"
            :disabled="proofreading || jobRunning"
            @click="handleProofread"
          >
            {{ proofreading || jobRunning ? "校对中..." : "开始校对" }}
          </button>
          <button
            class="ghost-button"
            :disabled="proofreading || jobRunning || !hasFailedBlocks"
            @click="handleRetryFailed"
          >
            重试失败块
          </button>
          <button
            class="ghost-button"
            @click="handleDeleteProject"
          >
            删除项目
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
      <p>稍后会展示项目元信息、问题面板和正文校对视图。</p>
    </div>

    <div
      v-else
      class="detail-layout"
    >
      <TiptapProofreadView
        v-if="projectDetail"
        ref="proofreadViewRef"
        :normalized-doc-path="projectDetail.normalizedDocPath"
        :issues="issues"
        :selected-issue-id="selectedIssueId"
        @page-context-change="handlePageContextChange"
      />

      <div class="side-panel">
        <InfoCard
          title="问题面板"
          subtitle="只显示校对视图当前页的问题与统计。"
        >
          <template #extra>
            <button
              class="panel-toggle"
              type="button"
              @click="issuePanelOpen = !issuePanelOpen"
            >
              {{ issuePanelOpen ? "收起" : "展开" }}
            </button>
          </template>

          <div
            v-if="issuePanelOpen"
            class="side-panel__body"
          >
          <div class="stats-inline">
            <article class="stats-inline__item">
              <span>总块数</span>
              <strong>{{ jobMetrics.total }}</strong>
            </article>
            <article class="stats-inline__item">
              <span>已完成</span>
              <strong>{{ jobMetrics.completed }}</strong>
            </article>
            <article class="stats-inline__item">
              <span>失败</span>
              <strong>{{ jobMetrics.failed }}</strong>
            </article>
            <article class="stats-inline__item">
              <span>当前页</span>
              <strong>{{ currentPageLabel }}</strong>
            </article>
            <article class="stats-inline__item">
              <span>状态</span>
              <strong>{{ jobStatusLabel }}</strong>
            </article>
          </div>

          <div
            v-if="panelMessage"
            class="success-banner"
          >
            {{ panelMessage }}
          </div>

          <div
            v-if="visibleIssues.length"
            class="issue-list"
          >
            <button
              v-for="issue in visibleIssues"
              :key="issue.id"
              class="issue-card"
              :class="{ 'issue-card--active': selectedIssueId === issue.id }"
              @click="handleSelectIssue(issue)"
            >
              <div class="project-card__header">
                <strong>{{ issue.quoteText }}</strong>
                <span class="project-pill">{{ issue.issueType }}</span>
              </div>
              <p class="project-card__meta">{{ issue.explanation }}</p>
              <div class="issue-card__footer">
                <span>建议：{{ issue.suggestion }}</span>
                <span>{{ issue.severity }}</span>
              </div>
            </button>
          </div>
          <div
            v-else
            class="placeholder-stack"
          >
            <div class="placeholder-row">
              <span class="status-dot"></span>
              <div>
                <strong>本页暂无问题</strong>
                <p>翻页后面板会自动刷新；如果整篇还未校对，先点击“开始校对”。</p>
              </div>
            </div>
          </div>
          </div>
        </InfoCard>

        <InfoCard
          title="调用日志"
          subtitle="只显示当前页 block 的模型调用状态、耗时与错误信息。"
        >
          <template #extra>
            <button
              class="panel-toggle"
              type="button"
              @click="callPanelOpen = !callPanelOpen"
            >
              {{ callPanelOpen ? "收起" : "展开" }}
            </button>
          </template>

          <div
            v-if="callPanelOpen"
            class="side-panel__body"
          >
          <div
            v-if="visibleCalls.length"
            class="call-list"
          >
            <article
              v-for="call in visibleCalls"
              :key="call.id"
              class="call-card"
            >
              <div class="project-card__header">
                <strong>{{ call.blockId }}</strong>
                <span
                  class="project-pill"
                  :class="{ 'project-pill--error': call.status === 'failed' }"
                >
                  {{ call.status }}
                </span>
              </div>
              <p class="project-card__meta">
                {{ call.modelName }} · {{ call.latencyMs ?? 0 }} ms
              </p>
              <p
                v-if="call.errorMessage"
                class="call-card__error"
              >
                {{ call.errorMessage }}
              </p>
            </article>
          </div>
          <div
            v-else
            class="placeholder-stack"
          >
            <div class="placeholder-row">
              <span class="status-dot status-dot--warn"></span>
              <div>
                <strong>本页暂无调用日志</strong>
                <p>翻页后面板会自动刷新；执行校对后，这里会展示当前页对应 block 的调用结果。</p>
              </div>
            </div>
          </div>
          </div>
        </InfoCard>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import InfoCard from "@/components/common/InfoCard.vue";
import TiptapProofreadView from "@/components/editor/TiptapProofreadView.vue";
import { deleteProject, getProjectDetail } from "@/api/projects";
import {
  getLatestProofreadingJob,
  listProofreadingCalls,
  listProofreadingIssues,
  startProofreading,
} from "@/api/proofreading";
import type {
  ProjectDetail,
  ProofreadOptions,
  ProofreadingCall,
  ProofreadingIssue,
  ProofreadingJob,
} from "@/types/models";
import { isTauriApp } from "@/utils/runtime";

type ProofreadViewExpose = {
  focusIssue: () => Promise<void>;
  scrollToBlock: (blockId: string) => Promise<void>;
};

const route = useRoute();
const router = useRouter();
const proofreadViewRef = ref<ProofreadViewExpose | null>(null);
const loading = ref(false);
const proofreading = ref(false);
const loadError = ref("");
const panelMessage = ref("");
const selectedIssueId = ref<string | null>(null);
const projectDetail = ref<ProjectDetail | null>(null);
const job = ref<ProofreadingJob | null>(null);
const issues = ref<ProofreadingIssue[]>([]);
const calls = ref<ProofreadingCall[]>([]);
const currentPage = ref(1);
const totalPages = ref(1);
const visibleBlockIds = ref<string[]>([]);
const issuePanelOpen = ref(true);
const callPanelOpen = ref(false);

const projectId = computed(() => String(route.params.id ?? ""));
const projectTitle = computed(() => projectDetail.value?.name ?? projectId.value);
const hasFailedBlocks = computed(() => (job.value?.failedBlocks ?? 0) > 0);
const visibleBlockIdSet = computed(() => new Set(visibleBlockIds.value));
const visibleIssues = computed(() =>
  issues.value.filter((issue) => visibleBlockIdSet.value.has(issue.blockId)),
);
const visibleCalls = computed(() =>
  calls.value.filter((call) => visibleBlockIdSet.value.has(call.blockId)),
);
const currentPageLabel = computed(() =>
  totalPages.value > 1 ? `${currentPage.value} / ${totalPages.value}` : "1",
);
const jobMetrics = computed(() => ({
  total: job.value?.totalBlocks ?? projectDetail.value?.totalBlocks ?? 0,
  completed: job.value?.completedBlocks ?? projectDetail.value?.completedBlocks ?? 0,
  failed: job.value?.failedBlocks ?? projectDetail.value?.failedBlocks ?? 0,
}));
const jobRunning = computed(() => job.value?.status === "running");
const jobStatusLabel = computed(() => {
  if (!job.value) return "未开始";
  if (job.value.status === "running") return "进行中";
  if (job.value.status === "completed") return "已完成";
  if (job.value.status === "failed") return "失败";
  return "待处理";
});
let pollingTimer: number | null = null;

const defaultOptions: ProofreadOptions = {
  mode: "full",
  maxChunkChars: 1200,
  overlapChars: 80,
  issueTypes: [
    "typo",
    "grammar",
    "wording",
    "redundancy",
    "consistency",
  ],
};

onMounted(() => {
  void loadInitialData();
  pollingTimer = window.setInterval(() => {
    if (jobRunning.value) {
      void refreshProofreadingData();
    }
  }, 5000);
});

onBeforeUnmount(() => {
  if (pollingTimer !== null) {
    window.clearInterval(pollingTimer);
    pollingTimer = null;
  }
});

async function loadInitialData() {
  await loadProjectDetail();
  await refreshProofreadingData();
}

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
    loadError.value = extractMessage(error, "项目详情加载失败。");
  } finally {
    loading.value = false;
  }
}

async function refreshProofreadingData() {
  if (!isTauriApp()) {
    return;
  }

  try {
    const [latestJob, issueList, callList] = await Promise.all([
      getLatestProofreadingJob(projectId.value),
      listProofreadingIssues(projectId.value),
      listProofreadingCalls(projectId.value),
    ]);
    job.value = latestJob;
    issues.value = issueList;
    calls.value = callList;
    if (!selectedIssueId.value && issueList.length) {
      selectedIssueId.value = issueList[0].id;
    }
  } catch (error) {
    loadError.value = extractMessage(error, "校对数据加载失败。");
  }
}

async function handleProofread() {
  if (!isTauriApp()) {
    loadError.value = "请通过 Tauri 桌面环境执行校对。";
    return;
  }

  proofreading.value = true;
  loadError.value = "";
  panelMessage.value = "";

  try {
    await executeProofreading(defaultOptions, "校对任务已启动，后台处理中。");
  } catch (error) {
    loadError.value = extractMessage(error, "校对任务执行失败。");
  } finally {
    proofreading.value = false;
  }
}

async function handleRetryFailed() {
  if (!isTauriApp()) {
    loadError.value = "请通过 Tauri 桌面环境执行重试。";
    return;
  }

  proofreading.value = true;
  loadError.value = "";
  panelMessage.value = "";

  try {
    await executeProofreading(
      {
        ...defaultOptions,
        mode: "retry_failed",
      },
      "失败块重试任务已启动，后台处理中。",
    );
  } catch (error) {
    loadError.value = extractMessage(error, "失败块重试失败。");
  } finally {
    proofreading.value = false;
  }
}

async function handleDeleteProject() {
  if (!projectDetail.value) {
    return;
  }

  if (!isTauriApp()) {
    loadError.value = "请通过 Tauri 桌面环境删除项目。";
    return;
  }

  const confirmed = window.confirm(
    `确认删除项目「${projectDetail.value.name}」吗？正在进行中的校对也会被直接终止，本地数据会被全部移除。`,
  );
  if (!confirmed) {
    return;
  }

  proofreading.value = true;
  loadError.value = "";

  try {
    await deleteProject(projectId.value);
    await router.push("/");
  } catch (error) {
    loadError.value = extractMessage(error, "删除项目失败。");
  } finally {
    proofreading.value = false;
  }
}

async function handleSelectIssue(issue: ProofreadingIssue) {
  selectedIssueId.value = issue.id;
  await proofreadViewRef.value?.scrollToBlock(issue.blockId);
  await proofreadViewRef.value?.focusIssue();
}

function handlePageContextChange(payload: {
  currentPage: number;
  totalPages: number;
  blockIds: string[];
}) {
  currentPage.value = payload.currentPage;
  totalPages.value = payload.totalPages;
  visibleBlockIds.value = payload.blockIds;
}

async function executeProofreading(
  options: ProofreadOptions,
  successMessage: string,
) {
  job.value = await startProofreading(projectId.value, options);
  await refreshProofreadingData();
  panelMessage.value = successMessage;
  if (issues.value.length) {
    await handleSelectIssue(issues.value[0]);
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
