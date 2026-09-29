// @vitest-environment happy-dom
// @vitest-environment-options {"happyDOM":{"settings":{"disableCSSFileLoading":true}}}
import { describe, expect, it } from "vitest";
import { presentationDocument } from "./presentation-document";
import type { PresentationView } from "./generated/PresentationView";

function view(): PresentationView {
  return { animation_assets: {}, restored_state: null, restored_state_revision: null, reference: { presentation_id: "p", revision: 2 }, title: "Recall", entrypoint: "page/index.html",
    content_files: { "page/index.html": '<link rel="stylesheet" href="../style.css"><h1>Recall</h1><script src="../main.js"></script><img src="../diagram.svg">',
      "style.css": ".card { padding: 8px; }", "main.js": "window.count = 2;", "diagram.svg": '<svg xmlns="http://www.w3.org/2000/svg"/>' },
    readable_view: { parts: [{ kind: "markdown", text: "Recall" }], sources: [] }, sources: [], assumptions: [], initial_state: { count: 2 } };
}
describe("presentation document", () => {
  it("resolves saved video, source and poster assets without embedding them in authored text", () => {
    const saved = view();
    saved.content_files[saved.entrypoint] = '<video src="../assets/animation-a.mp4" poster="../assets/animation-a.png"></video><video><source src="../assets/animation-a.mp4" type="video/mp4"></video>';
    saved.animation_assets = { "animation-a": {video_base64:"VIDEO",poster_png_base64:"POSTER",width:640,height:360,duration_seconds:2,fps:30,cues:[]} };
    const output = presentationDocument(saved);
    expect(output).toContain('src="data:video/mp4;base64,VIDEO"');
    expect(output).toContain('poster="data:image/png;base64,POSTER"');
    expect(output).toContain('media-src data:');
    expect(saved.content_files[saved.entrypoint]).not.toContain('VIDEO');
    delete saved.animation_assets['animation-a'];
    expect(() => presentationDocument(saved)).toThrow('动画资源缺失');
  });
  it("bundles the exact version's local assets, common style, initial values and isolated bridge", () => {
    const output = presentationDocument(view());
    const doc = new DOMParser().parseFromString(output, "text/html");
    expect(doc.querySelector("script[src]")).toBeNull();
    expect(doc.querySelector('link[rel="stylesheet"]')).toBeNull();
    expect(doc.querySelector("img")?.getAttribute("src")).toMatch(/^data:image\/svg\+xml/);
    expect(output).toContain("window.count = 2;");
    expect(output).toContain('"initialState":{"count":2}');
    expect(doc.head.firstElementChild?.getAttribute("content")).toContain("connect-src 'none'");
    expect(doc.documentElement.hasAttribute("data-presentation-pending")).toBe(true);
  });
  it("loads a managed library from the saved version once before business code", () => {
    const saved = view();
    saved.entrypoint = "index.html";
    saved.content_files = {
      "index.html": '<script data-presentation-library="konva" src="libraries/konva-10.7.0.min.js"></script><script>window.sceneVersion = Konva.version</script>',
      "libraries/konva-10.7.0.min.js": 'window.Konva = {version:"10.7.0"};',
      "libraries/konva-10.7.0.LICENSE.txt": "MIT"
    };
    const output = presentationDocument(saved);
    const doc = new DOMParser().parseFromString(output, "text/html");
    expect(doc.querySelectorAll("script[data-presentation-library]")).toHaveLength(1);
    expect(doc.querySelector("script[src]")).toBeNull();
    expect(output.indexOf('window.Konva =')).toBeLessThan(output.indexOf('window.sceneVersion ='));
    expect(output).toContain('version:"10.7.0"');
    expect(saved.content_files["index.html"]).toContain('src="libraries/');
  });
  it("reports missing and external assets instead of silently rendering a partial page", () => {
    const missing = view(); delete missing.content_files["main.js"];
    expect(() => presentationDocument(missing)).toThrow("内容资源缺失");
    const external = view(); external.content_files["page/index.html"] = '<script src="https://example.com/app.js"></script>';
    expect(() => presentationDocument(external)).toThrow("版本内的本地资源");
    const wrongType = view(); wrongType.content_files["page/index.html"] = '<img src="../plot.png">';
    expect(() => presentationDocument(wrongType)).toThrow("只支持 SVG");
  });
});
