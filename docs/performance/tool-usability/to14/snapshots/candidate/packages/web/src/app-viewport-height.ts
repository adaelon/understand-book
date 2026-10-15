import { WORKSPACE_LIMITS } from './workspace-layout';

export const APP_VIEWPORT_HEIGHT_PROPERTY = "--app-viewport-height";

export interface AppViewportHeightMeasurement {
  supportsDynamicViewport: boolean;
  innerHeight: number;
  visualHeight?: number | null;
  scale?: number | null;
}

function positiveFinite(value: number | null | undefined): number {
  return typeof value === "number" && Number.isFinite(value) && value > 0 ? value : 0;
}

export function resolveLegacyAppViewportHeight(
  measurement: AppViewportHeightMeasurement,
): number | null {
  if (measurement.supportsDynamicViewport) return null;

  const innerHeight = positiveFinite(measurement.innerHeight);
  const visualHeight = positiveFinite(measurement.visualHeight);
  const scale = positiveFinite(measurement.scale) || 1;
  if (!visualHeight) return innerHeight || null;

  const unscaledVisualHeight = visualHeight * scale;
  return innerHeight ? Math.min(innerHeight, unscaledVisualHeight) : unscaledVisualHeight;
}

export function installAppViewportHeightFallback(
  targetWindow: Window = window,
  targetDocument: Document = document,
  supportsDynamicViewport = typeof CSS !== "undefined" && CSS.supports("height", "100dvh"),
): () => void {
  const rootStyle = targetDocument.documentElement.style;
  let keyboardVisible = false;

  const update = () => {
    const visualViewport = targetWindow.visualViewport;
    const height = resolveLegacyAppViewportHeight({
      supportsDynamicViewport,
      innerHeight: targetWindow.innerHeight,
      visualHeight: visualViewport?.height,
      scale: visualViewport?.scale,
    });
    if (height) rootStyle.setProperty(APP_VIEWPORT_HEIGHT_PROPERTY, `${height}px`);
    else rootStyle.removeProperty(APP_VIEWPORT_HEIGHT_PROPERTY);

    // dvh follows browser chrome, but iOS keyboards only resize the visual viewport.
    // Keep the input in place through a send-button focus change until the keyboard closes.
    const workspaceInput = !!targetDocument.activeElement?.closest('[data-workspace-input]');
    keyboardVisible = !!visualViewport && (workspaceInput || keyboardVisible)
      && Math.abs(visualViewport.scale - 1) < 0.01
      && targetWindow.innerHeight - visualViewport.height >= WORKSPACE_LIMITS.keyboardHeightLoss;
    if (keyboardVisible && visualViewport) {
      targetDocument.documentElement.dataset.workspaceKeyboard = 'true';
      rootStyle.setProperty('--app-input-viewport-height', `${visualViewport.height}px`);
      rootStyle.setProperty('--app-input-viewport-top', `${Math.max(0, visualViewport.offsetTop)}px`);
    } else {
      delete targetDocument.documentElement.dataset.workspaceKeyboard;
      rootStyle.removeProperty('--app-input-viewport-height');
      rootStyle.removeProperty('--app-input-viewport-top');
    }
  };

  const visualViewport = targetWindow.visualViewport;
  update();
  targetWindow.addEventListener("resize", update, { passive: true });
  targetDocument.addEventListener('focusin', update);
  visualViewport?.addEventListener("resize", update, { passive: true });
  visualViewport?.addEventListener("scroll", update, { passive: true });

  return () => {
    targetWindow.removeEventListener("resize", update);
    targetDocument.removeEventListener('focusin', update);
    visualViewport?.removeEventListener("resize", update);
    visualViewport?.removeEventListener("scroll", update);
    rootStyle.removeProperty(APP_VIEWPORT_HEIGHT_PROPERTY);
    delete targetDocument.documentElement.dataset.workspaceKeyboard;
    rootStyle.removeProperty('--app-input-viewport-height');
    rootStyle.removeProperty('--app-input-viewport-top');
  };
}
