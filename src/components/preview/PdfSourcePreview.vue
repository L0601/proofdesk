<template>
  <InfoCard
    title="原文对照"
    subtitle="使用 PDF.js 渲染原始 PDF，当前支持基础翻页。"
  >
    <div
      v-if="loading"
      class="empty-state"
    >
      <strong>正在加载 PDF...</strong>
      <p>首次打开时会初始化 PDF.js。</p>
    </div>
    <div
      v-else-if="errorMessage"
      class="error-banner"
    >
      {{ errorMessage }}
    </div>
    <template v-else>
      <div class="pdf-toolbar">
        <button
          class="ghost-button"
          :disabled="pageNumber <= 1"
          @click="pageNumber -= 1"
        >
          上一页
        </button>
        <span class="inline-note">第 {{ pageNumber }} / {{ pageCount }} 页</span>
        <button
          class="ghost-button"
          :disabled="pageNumber >= pageCount"
          @click="pageNumber += 1"
        >
          下一页
        </button>
      </div>
      <div class="pdf-canvas-wrap">
        <canvas ref="canvasRef"></canvas>
      </div>
    </template>
  </InfoCard>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import InfoCard from "@/components/common/InfoCard.vue";

type PdfJsLib = typeof import("pdfjs-dist");
type PdfDocument = Awaited<ReturnType<PdfJsLib["getDocument"]>["promise"]>;

const props = defineProps<{
  filePath: string;
}>();

const canvasRef = ref<HTMLCanvasElement | null>(null);
const loading = ref(false);
const errorMessage = ref("");
const pageNumber = ref(1);
const pageCount = ref(1);
const pdfDocument = ref<PdfDocument | null>(null);

onMounted(() => {
  void loadPdf();
});

watch(
  () => props.filePath,
  () => {
    pageNumber.value = 1;
    void loadPdf();
  },
);

watch(pageNumber, () => {
  void renderCurrentPage();
});

async function loadPdf() {
  if (!props.filePath) {
    return;
  }

  loading.value = true;
  errorMessage.value = "";

  try {
    const pdfjs = await import("pdfjs-dist");
    pdfjs.GlobalWorkerOptions.workerSrc = new URL(
      "pdfjs-dist/build/pdf.worker.mjs",
      import.meta.url,
    ).toString();
    pdfDocument.value = await pdfjs.getDocument(props.filePath).promise;
    pageCount.value = pdfDocument.value.numPages;
    await renderCurrentPage();
  } catch (error) {
    errorMessage.value = extractMessage(error);
  } finally {
    loading.value = false;
  }
}

async function renderCurrentPage() {
  if (!pdfDocument.value || !canvasRef.value) {
    return;
  }

  const page = await pdfDocument.value.getPage(pageNumber.value);
  const viewport = page.getViewport({ scale: 1.2 });
  const canvas = canvasRef.value;
  const context = canvas.getContext("2d");
  if (!context) {
    return;
  }

  canvas.width = viewport.width;
  canvas.height = viewport.height;

  await page.render({
    canvasContext: context,
    viewport,
  }).promise;
}

function extractMessage(error: unknown) {
  if (typeof error === "string") {
    return error;
  }

  if (error && typeof error === "object" && "message" in error) {
    return String(error.message);
  }

  return "PDF 预览加载失败。";
}
</script>
