import { createRouter, createWebHistory } from "vue-router";
import ProjectListPage from "@/pages/ProjectListPage.vue";
import ProjectDetailPage from "@/pages/ProjectDetailPage.vue";
import SettingsPage from "@/pages/SettingsPage.vue";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/",
      name: "project-list",
      component: ProjectListPage,
    },
    {
      path: "/project/:id",
      name: "project-detail",
      component: ProjectDetailPage,
    },
    {
      path: "/settings",
      name: "settings",
      component: SettingsPage,
    },
  ],
});

export default router;
