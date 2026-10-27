import { installAppViewportHeightFallback } from "./app-viewport-height";
import { createApp } from "vue";
import MobileWorkspaceFixture from "./components/MobileWorkspaceFixture.vue";
import "./style.css";
import "katex/dist/katex.min.css";

installAppViewportHeightFallback();
createApp(MobileWorkspaceFixture).mount("#app");

