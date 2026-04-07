<template>
  <InfoCard
    title="校对视图"
    subtitle="正文来自标准化文档模型，问题会按 block 与 offset 高亮到正文里。"
  >
    <div
      v-if="loading"
      class="empty-state"
    >
      <strong>正在加载正文...</strong>
      <p>系统正在读取标准化文档 JSON。</p>
    </div>
    <div
      v-else-if="errorMessage"
      class="error-banner"
    >
      {{ errorMessage }}
    </div>
    <EditorContent
      v-else-if="editor"
      class="editor-surface"
      :editor="editor"
    />
  </InfoCard>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Editor, EditorContent } from "@tiptap/vue-3";
import StarterKit from "@tiptap/starter-kit";
import Underline from "@tiptap/extension-underline";
import UniqueID from "@tiptap/extension-unique-id";
import { readTextFile } from "@tauri-apps/plugin-fs";
import InfoCard from "@/components/common/InfoCard.vue";
import IssueMark from "@/components/editor/extensions/IssueMark";
import type { NormalizedDocument, ProofreadingIssue } from "@/types/models";
import { buildEditorDoc } from "@/utils/buildEditorDoc";
import { isTauriApp } from "@/utils/runtime";

const props = defineProps<{
  normalizedDocPath: string;
  issues?: ProofreadingIssue[];
  selectedIssueId?: string | null;
}>();

const editor = ref<Editor | null>(null);
const loading = ref(false);
const errorMessage = ref("");
const currentDocument = ref<NormalizedDocument | null>(null);

onMounted(() => {
  void loadDocument();
});

watch(
  () => props.normalizedDocPath,
  () => {
    void loadDocument();
  },
);

watch(
  () => props.issues,
  () => {
    rerender();
  },
  { deep: true },
);

watch(
  () => props.selectedIssueId,
  () => {
    void focusIssue();
  },
);

onBeforeUnmount(() => {
  editor.value?.destroy();
});

defineExpose({
  scrollToBlock,
  focusIssue,
});

async function loadDocument() {
  if (!props.normalizedDocPath) {
    return;
  }

  if (!isTauriApp()) {
    errorMessage.value = "请通过 Tauri 桌面环境查看正文。";
    return;
  }

  loading.value = true;
  errorMessage.value = "";

  try {
    const raw = await readTextFile(props.normalizedDocPath);
    currentDocument.value = JSON.parse(raw) as NormalizedDocument;
    rerender();
    await focusIssue();
  } catch (error) {
    errorMessage.value = extractMessage(error);
  } finally {
    loading.value = false;
  }
}

function rerender() {
  if (!currentDocument.value) {
    return;
  }

  const content = buildEditorDoc(currentDocument.value, props.issues ?? []);

  if (!editor.value) {
    editor.value = new Editor({
      editable: false,
      extensions: [
        StarterKit,
        Underline,
        IssueMark,
        UniqueID.configure({
          attributeName: "data-block-id",
          types: ["paragraph", "heading"],
        }),
      ],
      content,
    });
    return;
  }

  editor.value.commands.setContent(content);
}

async function focusIssue() {
  if (!props.selectedIssueId || !editor.value) {
    return;
  }

  await nextTick();
  const root = editor.value.view.dom as HTMLElement;
  root.querySelectorAll(".issue-mark--active").forEach((element) => {
    element.classList.remove("issue-mark--active");
  });

  const target = root.querySelector<HTMLElement>(
    `[data-issue-id="${props.selectedIssueId}"]`,
  );
  if (!target) {
    return;
  }

  target.classList.add("issue-mark--active");
  target.scrollIntoView({ behavior: "smooth", block: "center" });
}

async function scrollToBlock(blockId: string) {
  if (!editor.value) {
    return;
  }

  await nextTick();
  const target = (editor.value.view.dom as HTMLElement).querySelector<HTMLElement>(
    `[data-block-id="${blockId}"]`,
  );
  target?.scrollIntoView({ behavior: "smooth", block: "center" });
}

function extractMessage(error: unknown) {
  if (typeof error === "string") {
    return error;
  }

  if (error && typeof error === "object" && "message" in error) {
    return String(error.message);
  }

  return "正文加载失败。";
}
</script>
