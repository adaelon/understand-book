// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import {
  APP_VIEWPORT_HEIGHT_PROPERTY,
  installAppViewportHeightFallback,
  resolveLegacyAppViewportHeight,
} from "./app-viewport-height";

class FakeVisualViewport extends EventTarget {
  width = 390;
  height = 760;
  offsetTop = 0;
  offsetLeft = 0;
  pageTop = 0;
  pageLeft = 0;
  scale = 1;
}

afterEach(() => {
  document.documentElement.style.removeProperty(APP_VIEWPORT_HEIGHT_PROPERTY);
  document.body.replaceChildren();
});

describe("legacy app viewport height", () => {
  it('fits a workspace input above the keyboard even with dvh, and restores after keyboard dismissal', () => {
    const visualViewport = new FakeVisualViewport();
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 669 });
    Object.defineProperty(window, 'visualViewport', { configurable: true, value: visualViewport });
    visualViewport.height = 669;
    const input = document.createElement('textarea'); input.dataset.workspaceInput = 'agent'; document.body.append(input);
    const stop = installAppViewportHeightFallback(window, document, true);
    try {
      input.focus();
      visualViewport.height = 350; visualViewport.offsetTop = 80;
      visualViewport.dispatchEvent(new Event('resize'));
      const root = document.documentElement;
      expect(root.dataset.workspaceKeyboard).toBe('true');
      expect(root.style.getPropertyValue('--app-input-viewport-height')).toBe('350px');
      expect(root.style.getPropertyValue('--app-input-viewport-top')).toBe('80px');
      input.blur(); // Clicking send must not move it away while the keyboard is still visible.
      visualViewport.dispatchEvent(new Event('scroll'));
      expect(root.dataset.workspaceKeyboard).toBe('true');
      visualViewport.height = 669; visualViewport.offsetTop = 0;
      visualViewport.dispatchEvent(new Event('resize'));
      expect(root.dataset.workspaceKeyboard).toBeUndefined();
      expect(root.style.getPropertyValue('--app-input-viewport-height')).toBe('');
    } finally { stop(); }
  });

  it('does not treat pinch zoom or an unrelated input as a workspace keyboard', () => {
    const visualViewport = new FakeVisualViewport();
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 669 });
    Object.defineProperty(window, 'visualViewport', { configurable: true, value: visualViewport });
    visualViewport.height = 335;
    const input = document.createElement('textarea'); document.body.append(input); input.focus();
    const stop = installAppViewportHeightFallback(window, document, true);
    try {
      expect(document.documentElement.dataset.workspaceKeyboard).toBeUndefined();
      input.dataset.workspaceInput = 'agent'; visualViewport.scale = 2;
      visualViewport.dispatchEvent(new Event('resize'));
      expect(document.documentElement.dataset.workspaceKeyboard).toBeUndefined();
    } finally { stop(); }
  });
  it("leaves dynamic viewport sizing to CSS when dvh is supported", () => {
    expect(resolveLegacyAppViewportHeight({
      supportsDynamicViewport: true,
      innerHeight: 844,
      visualHeight: 760,
      scale: 1,
    })).toBeNull();
  });

  it("uses the unobscured visual height when a legacy toolbar covers 100vh", () => {
    expect(resolveLegacyAppViewportHeight({
      supportsDynamicViewport: false,
      innerHeight: 844,
      visualHeight: 760,
      scale: 1,
    })).toBe(760);
  });

  it("does not shrink the app layout in response to pinch zoom", () => {
    expect(resolveLegacyAppViewportHeight({
      supportsDynamicViewport: false,
      innerHeight: 844,
      visualHeight: 422,
      scale: 2,
    })).toBe(844);
  });

  it("updates the CSS height when a legacy visual viewport changes", () => {
    const visualViewport = new FakeVisualViewport();
    Object.defineProperty(window, "innerHeight", { configurable: true, value: 844 });
    Object.defineProperty(window, "visualViewport", { configurable: true, value: visualViewport });

    const stop = installAppViewportHeightFallback(window, document, false);
    expect(document.documentElement.style.getPropertyValue(APP_VIEWPORT_HEIGHT_PROPERTY)).toBe("760px");

    visualViewport.height = 700;
    visualViewport.dispatchEvent(new Event("resize"));
    expect(document.documentElement.style.getPropertyValue(APP_VIEWPORT_HEIGHT_PROPERTY)).toBe("700px");

    stop();
    expect(document.documentElement.style.getPropertyValue(APP_VIEWPORT_HEIGHT_PROPERTY)).toBe("");
  });
});
