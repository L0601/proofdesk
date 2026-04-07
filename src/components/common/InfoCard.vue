<template>
  <section class="info-card">
    <header
      v-if="title || subtitle"
      class="info-card__header"
    >
      <div>
        <h3
          v-if="title"
          class="info-card__title"
        >
          {{ title }}
        </h3>
        <p
          v-if="subtitle"
          class="info-card__subtitle"
        >
          {{ subtitle }}
        </p>
      </div>
      <slot name="extra" />
    </header>
    <div class="info-card__body">
      <slot />
    </div>
  </section>
</template>

<script setup lang="ts">
defineProps<{
  title?: string;
  subtitle?: string;
}>();
</script>
