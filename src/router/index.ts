import { createRouter, createWebHistory } from "vue-router";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/",
      name: "project-list",
      component: () => import("@/pages/ProjectListPage.vue"),
    },
    {
      path: "/project/:id",
      name: "project-detail",
      component: () => import("@/pages/ProjectDetailPage.vue"),
    },
    {
      path: "/settings",
      name: "settings",
      component: () => import("@/pages/SettingsPage.vue"),
    },
  ],
});

export default router;
