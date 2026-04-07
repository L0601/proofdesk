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
        </div>
        <div class="metric-group">
          <article class="metric-tile">
            <span>项目数</span>
            <strong>0</strong>
          </article>
          <article class="metric-tile">
            <span>进行中</span>
            <strong>0</strong>
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
        title="近期规划"
        subtitle="先把可运行底座做稳，再挂导入与校对能力。"
      >
        <ul class="feature-list">
          <li>工程骨架与统一视觉系统</li>
          <li>SQLite migration 与 repository 骨架</li>
          <li>DOCX / PDF 导入链路</li>
          <li>AI 校对任务调度与进度事件</li>
        </ul>
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
import InfoCard from "@/components/common/InfoCard.vue";
</script>
