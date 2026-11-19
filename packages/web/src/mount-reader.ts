import { t as uiText } from './i18n';
import { createApp, h, type Component } from "vue";
import ApplicationShell from "./ApplicationShell.vue";
import { installAppViewportHeightFallback } from "./app-viewport-height";
import "katex/dist/katex.min.css"; // agent 答案 LaTeX 公式样式
import "./style.css";
import { initializeLanguage } from './i18n';
import { initializeDesktopLanguage } from './desktop-language';
import { initializeNoteDraftStorage } from './personal-notes';



export async function mountReader(entry: Component) {
  initializeLanguage(uiText('understand-book 阅读器'));
  initializeDesktopLanguage();
  // Keep the inline startup animation visible while production styles download.
  const styles = Array.from(document.querySelectorAll<HTMLLinkElement>("link[data-app-styles]"));
  await Promise.all(styles.map(style => new Promise<void>(resolve => {
    style.addEventListener("load", () => resolve(), { once: true });
    style.addEventListener("error", () => resolve(), { once: true });
    style.media = "print";
    style.rel = "stylesheet";
  })));
  styles.forEach(style => { style.media = "all"; });
  await initializeNoteDraftStorage();
  const stopViewportHeightFallback = installAppViewportHeightFallback();
  createApp({ render: () => h(ApplicationShell, {}, () => h(entry)) }).mount("#app");
  if (import.meta.hot) import.meta.hot.dispose(stopViewportHeightFallback);
}

