<template>
  <section class="page-grid">
    <InfoCard
      title="项目详情"
      subtitle="这里会承接项目元信息、进度、调用日志、问题列表和正文联动。"
    >
      <div class="detail-banner">
        <div>
          <p class="eyebrow">项目 ID</p>
          <h1 class="hero-title">{{ projectId }}</h1>
        </div>
        <RouterLink
          class="ghost-link"
          to="/"
        >
          返回项目列表
        </RouterLink>
      </div>
    </InfoCard>

    <div class="detail-layout">
      <TiptapDemo />

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
import { computed } from "vue";
import { useRoute } from "vue-router";
import InfoCard from "@/components/common/InfoCard.vue";
import TiptapDemo from "@/components/editor/TiptapDemo.vue";

const route = useRoute();

const projectId = computed(() => String(route.params.id ?? ""));
</script>
