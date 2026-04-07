<template>
  <section class="page-grid">
    <InfoCard
      title="设置"
      subtitle="后续会配置 OpenAI-compatible 接口参数、并发数与提示词模板。"
    >
      <div class="settings-grid">
        <label class="field">
          <span>Base URL</span>
          <input
            type="text"
            value="https://api.example.com/v1"
            disabled
          />
        </label>
        <label class="field">
          <span>Model</span>
          <input
            type="text"
            value="gpt-4.1-mini"
            disabled
          />
        </label>
        <label class="field">
          <span>Timeout</span>
          <input
            type="text"
            value="60000"
            disabled
          />
        </label>
        <label class="field">
          <span>Max Concurrency</span>
          <input
            type="text"
            value="4"
            disabled
          />
        </label>
      </div>
    </InfoCard>
  </section>
</template>

<script setup lang="ts">
import InfoCard from "@/components/common/InfoCard.vue";
</script>
