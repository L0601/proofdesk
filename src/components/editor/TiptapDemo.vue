<template>
  <InfoCard
    title="正文预览"
    subtitle="当前用于验证 Tiptap 依赖与基础样式接入。"
  >
    <EditorContent
      v-if="editor"
      class="editor-surface"
      :editor="editor"
    />
  </InfoCard>
</template>

<script setup lang="ts">
import { EditorContent, useEditor } from "@tiptap/vue-3";
import StarterKit from "@tiptap/starter-kit";
import UniqueID from "@tiptap/extension-unique-id";
import InfoCard from "@/components/common/InfoCard.vue";

const editor = useEditor({
  editable: false,
  extensions: [
    StarterKit,
    UniqueID.configure({
      attributeName: "data-block-id",
      types: ["paragraph", "heading"],
    }),
  ],
  content: {
    type: "doc",
    content: [
      {
        type: "heading",
        attrs: { level: 2 },
        content: [{ type: "text", text: "ProofDesk 编辑器挂载成功" }],
      },
      {
        type: "paragraph",
        attrs: { id: "demo-block-1" },
        content: [
          { type: "text", text: "这里会承接标准化文档模型的正文内容。" },
        ],
      },
      {
        type: "paragraph",
        attrs: { id: "demo-block-2" },
        content: [
          { type: "text", text: "后续会在这里叠加 issue 标记、跳转和滚动定位能力。" },
        ],
      },
    ],
  },
});
</script>
