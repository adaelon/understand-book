import { createApp } from "vue";
import App from "./App.vue";
import NetworkApp from "./NetworkApp.vue";
import { installAppViewportHeightFallback } from "./app-viewport-height";
import "katex/dist/katex.min.css"; // agent 答案 LaTeX 公式样式
import "./style.css";

async function mountReader() {
  // Keep the inline startup animation visible while production styles download.
  const styles = Array.from(document.querySelectorAll<HTMLLinkElement>("link[data-app-styles]"));
  await Promise.all(styles.map(style => new Promise<void>(resolve => {
    style.addEventListener("load", () => resolve(), { once: true });
    style.addEventListener("error", () => resolve(), { once: true });
    style.media = "print";
    style.rel = "stylesheet";
  })));
  styles.forEach(style => { style.media = "all"; });
  const stopViewportHeightFallback = installAppViewportHeightFallback();
  createApp(import.meta.env.VITE_MULTI_USER === "1" ? NetworkApp : App).mount("#app");
  if (import.meta.hot) import.meta.hot.dispose(stopViewportHeightFallback);
}

void mountReader();
