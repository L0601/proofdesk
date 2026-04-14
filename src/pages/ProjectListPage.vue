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
              {{ loading ? "导入中..." : "导入文档" }}
            </button>
            <span class="inline-note">
              当前支持 `.docx` 与文本型 `.pdf`，扫描件会被拒绝
            </span>
          </div>
          <input
            ref="webFileInput"
            type="file"
            accept=".docx,.pdf,application/pdf,application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            class="visually-hidden"
            @change="handleWebFileChange"
          />
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
        <details v-if="debugLogs.length" class="debug-panel">
          <summary>导入调试日志</summary>
          <pre class="debug-panel__content">{{ debugLogsText }}</pre>
        </details>

        <div
          v-if="projects.length"
          class="project-list"
        >
          <article
            v-for="project in projects"
            :key="project.id"
            class="project-card"
          >
            <div class="project-card__header">
              <RouterLink :to="`/project/${project.id}`">
                <strong>{{ project.name }}</strong>
              </RouterLink>
              <span class="project-pill">{{ project.sourceType }}</span>
            </div>
            <p class="project-card__meta">{{ project.sourceFileName }}</p>
            <div class="project-card__footer">
              <span>{{ project.totalBlocks }} 个段落</span>
              <span>{{ project.status }}</span>
              <button
                class="ghost-button"
                :disabled="loading || project.status === 'processing'"
                @click="handleDeleteProject(project)"
              >
                删除
              </button>
            </div>
          </article>
        </div>

        <div
          v-else
          class="empty-state"
        >
          <strong>还没有项目</strong>
          <p>从本地选择 DOCX 或文本型 PDF，系统会自动生成标准化文档并保存。</p>
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
import { readFile } from "@tauri-apps/plugin-fs";
import InfoCard from "@/components/common/InfoCard.vue";
import { getAppSettings } from "@/api/settings";
import {
  deleteProject,
  importDocument,
  importNormalizedDocument,
  listProjects,
} from "@/api/projects";
import type { AppSettings, ProjectSummary } from "@/types/models";
import { extractPdfNormalizedDocument } from "@/utils/pdfImport";
import { isTauriApp } from "@/utils/runtime";

type SelectedFile =
  | { kind: "path"; filePath: string }
  | { kind: "file"; file: File };

const loading = ref(false);
const errorMessage = ref("");
const projects = ref<ProjectSummary[]>([]);
const debugLogs = ref<string[]>([]);
const webFileInput = ref<HTMLInputElement | null>(null);

const processingCount = computed(() =>
  projects.value.filter((item) => item.status === "processing").length,
);
const debugLogsText = computed(() => debugLogs.value.join("\n"));

onMounted(() => {
  void refreshProjects();
});

async function refreshProjects() {
  if (!isTauriApp()) {
    return;
  }

  projects.value = await listProjects();
}

async function handleDeleteProject(project: ProjectSummary) {
  if (!isTauriApp()) {
    errorMessage.value = "请通过 Tauri 桌面环境删除项目。";
    return;
  }

  if (project.status === "processing") {
    errorMessage.value = "项目正在后台处理中，暂不允许删除。";
    return;
  }

  const confirmed = window.confirm(`确认删除项目「${project.name}」吗？该项目的本地数据会被全部移除。`);
  if (!confirmed) {
    return;
  }

  loading.value = true;
  errorMessage.value = "";

  try {
    await deleteProject(project.id);
    await refreshProjects();
  } catch (error) {
    errorMessage.value = extractMessage(error);
  } finally {
    loading.value = false;
  }
}

async function handleImport() {
  resetImportFeedback();

  if (!isTauriApp()) {
    logImport("当前为纯前端调试环境，使用浏览器文件选择器");
    webFileInput.value?.click();
    return;
  }

  let selected: SelectedFile | null = null;

  try {
    const result = await open({
      multiple: false,
      filters: [{ name: "Document", extensions: ["docx", "pdf"] }],
    });

    if (!result || Array.isArray(result)) {
      logImport("用户取消了文件选择");
      return;
    }

    selected = { kind: "path", filePath: result };
    logImport("Tauri 文件选择完成", { filePath: result });
  } catch (error) {
    handleImportError("无法打开文件选择器", error);
    return;
  }

  await processSelectedFile(selected);
}

async function handleWebFileChange(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0];

  if (!file) {
    logImport("浏览器文件选择器未返回文件");
    return;
  }

  logImport("浏览器文件选择完成", {
    name: file.name,
    size: file.size,
    type: file.type,
  });
  await processSelectedFile({ kind: "file", file });
  input.value = "";
}

async function processSelectedFile(selected: SelectedFile) {
  loading.value = true;

  try {
    const fileName = getSelectedFileName(selected);
    logImport("开始处理导入文件", {
      fileName,
      mode: selected.kind,
    });

    if (fileName.toLowerCase().endsWith(".pdf")) {
      const normalized = await parsePdf(selected);
      if (selected.kind === "path") {
        await importNormalizedDocument(selected.filePath, "pdf", normalized);
        logImport("PDF 项目已写入本地数据库", {
          blockCount: normalized.blocks.length,
        });
      } else {
        logImport("纯前端模式下完成 PDF 解析，未写入本地数据库", {
          blockCount: normalized.blocks.length,
        });
        errorMessage.value =
          "当前为前端调试模式：PDF 已完成解析，但不会保存到本地项目库。";
      }
    } else {
      if (selected.kind !== "path") {
        throw new Error(
          "当前为前端调试模式，DOCX 导入依赖 Tauri 后端，暂不支持直接保存。",
        );
      }

      await importDocument(selected.filePath);
      logImport("DOCX 项目已写入本地数据库");
    }

    await refreshProjects();
  } catch (error) {
    handleImportError("导入出错了", error);
  } finally {
    loading.value = false;
  }
}

async function parsePdf(selected: SelectedFile) {
  const settings = await loadPdfImportSettings();

  if (selected.kind === "path") {
    logImport("开始读取 Tauri 本地文件", {
      filePath: selected.filePath,
    });
    const bytes = await readFile(selected.filePath);
    logImport("Tauri 本地文件读取完成", {
      byteLength: bytes.byteLength,
    });
    return extractPdfNormalizedDocument(
      bytes,
      { minBlockChars: settings.pdfMinBlockChars },
      logImport,
    );
  }

  logImport("开始读取浏览器文件内容", {
    name: selected.file.name,
    size: selected.file.size,
  });
  const bytes = new Uint8Array(await selected.file.arrayBuffer());
  logImport("浏览器文件读取完成", { byteLength: bytes.byteLength });
  return extractPdfNormalizedDocument(
    bytes,
    { minBlockChars: settings.pdfMinBlockChars },
    logImport,
  );
}

async function loadPdfImportSettings(): Promise<AppSettings> {
  if (!isTauriApp()) {
    return {
      baseUrl: "",
      apiKey: "",
      model: "",
      timeoutMs: 60000,
      maxConcurrency: 4,
      pdfMinBlockChars: 16,
      temperature: 0.2,
      maxTokens: 1200,
      systemPromptTemplate: "",
    };
  }

  return getAppSettings();
}

function getSelectedFileName(selected: SelectedFile) {
  if (selected.kind === "file") {
    return selected.file.name;
  }

  const segments = selected.filePath.split(/[\\/]/);
  return segments[segments.length - 1] ?? selected.filePath;
}

function resetImportFeedback() {
  errorMessage.value = "";
  debugLogs.value = [];
}

function handleImportError(prefix: string, error: unknown) {
  const detail = extractErrorDetail(error);
  errorMessage.value = `${prefix}：${detail.message}`;
  logImport(prefix, {
    message: detail.message,
    stack: detail.stack,
    raw: error,
  });
  console.error(`[proofdesk] ${prefix}`, error);
}

function logImport(message: string, payload?: unknown) {
  const line = payload
    ? `[${new Date().toLocaleTimeString("zh-CN", { hour12: false })}] ${message} ${safeStringify(payload)}`
    : `[${new Date().toLocaleTimeString("zh-CN", { hour12: false })}] ${message}`;
  debugLogs.value.push(line);
  console.log(`[proofdesk] ${message}`, payload ?? "");
}

function safeStringify(payload: unknown) {
  try {
    return JSON.stringify(payload, null, 2);
  } catch {
    return String(payload);
  }
}

function extractMessage(error: unknown) {
  return extractErrorDetail(error).message;
}

function extractErrorDetail(error: unknown) {
  if (typeof error === "string") {
    return { message: error, stack: null as string | null };
  }

  if (error && typeof error === "object" && "message" in error) {
    const stack =
      "stack" in error && typeof error.stack === "string" ? error.stack : null;
    return { message: String(error.message), stack };
  }

  return { message: "导入失败，请稍后重试。", stack: null as string | null };
}
</script>
