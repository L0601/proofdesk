<template>
  <div class="app-shell">
    <header class="topbar">
      <div class="brand">
        <div class="brand__badge">砚</div>
        <div>
          <p class="brand__title">砚准 ProofDesk</p>
          <p class="brand__subtitle">面向长文的本地智能校对台</p>
        </div>
      </div>
      <nav class="topbar__nav">
        <RouterLink
          v-for="item in navItems"
          :key="item.to"
          class="topbar__link"
          :to="item.to"
        >
          {{ item.label }}
        </RouterLink>
      </nav>
    </header>

    <main class="app-shell__content">
      <RouterView />
    </main>
  </div>
</template>

<script setup lang="ts">
const navItems = [
  { label: "项目", to: "/" },
  { label: "设置", to: "/settings" },
];
</script>
