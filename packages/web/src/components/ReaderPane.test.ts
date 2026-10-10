// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import type { MemoryRecord } from "../api";
import { renderFormulaSource, renderTableSource } from "../md";
import ReaderPane, { type Segment } from "./ReaderPane.vue";

function segment(lid: string, kind: Segment["kind"]): Segment {
  return { lid, kind, text: `text-${lid}`, formula: null, imageAsset: null };
}

function note(memId: string, lid: string, content: string): MemoryRecord {
  return {
    mem_id: memId,
    type: "note",
    layer: "long_term",
    book_id: "book-a",
    anchor: { lid },
    content,
  };
}

describe("ReaderPane Note rendering", () => {
  it("keeps a display formula between separate prose paragraphs", async () => {
    const formula: Segment = {
      ...segment("1.2", "formula"),
      text: "$$\n\\mathrm{Speedup}=\\frac{1}{(1-f)+f/s}.\n$$",
    };
    const wrapper = mount(ReaderPane, {
      props: {
        segments: [segment("1.1", "paragraph"), formula, segment("1.3", "paragraph")],
        viewportAnchor: null,
        selectedLid: null,
        renderSeg: (value) => value.kind === "formula" ? renderFormulaSource(value.text) : value.text,
        renderMarkdown: (source) => source,
        markdownHeadingLevel: () => null,
        isAsset: () => false,
        isHighlighted: () => false,
        highlightsOf: () => [],
        highlightCardsOf: () => [],
        visibleNotes: [],
        hlExcerpt: () => "",
        imageMeta: () => null,
        imageAsset: () => null,
      },
    });

    expect(wrapper.findAll(".flow-paragraph")).toHaveLength(2);
    expect(wrapper.get('[data-lid="1.2"]').element.closest("p")).toBeNull();
    expect(wrapper.find('[data-lid="1.2"] .katex-display').exists()).toBe(true);
    expect(wrapper.findAll(".seg")).toHaveLength(3);
    await wrapper.get('[data-lid="1.2"]').trigger("click");
    expect(wrapper.emitted("select")?.at(-1)).toEqual(["1.2"]);

    await wrapper.setProps({ segments: [segment("1.1", "paragraph"), {
      ...formula,
      formula: {} as NonNullable<Segment["formula"]>,
    }, segment("1.3", "paragraph")] });
    await wrapper.get('[data-lid="1.2"] button').trigger("click");
    expect(wrapper.emitted("open-formula")?.at(-1)?.[0]).toMatchObject({ lid: "1.2" });
    wrapper.unmount();
  });

  it("keeps passive anchor and selected states visually neutral", () => {
    const styles = readFileSync("src/style.css", "utf8");

    expect(styles).not.toContain(".prose p.anchor");
    expect(styles).not.toContain(".prose p.selected");
    expect(styles).not.toContain(".flow-text.anchor");
    expect(styles).not.toContain(".flow-text.selected");
    expect(styles).toContain(".prose p.hl");
    expect(styles).toContain(".flow-text.hl");
  });

  it("groups annotations outside prose and keeps one preview with original record actions", async () => {
    const short = note("note-short", "1.1", "> quoted source\n\nShort **body**");
    const long = note("note-long", "2.1", `> long source\n\n${"x".repeat(400)}`);
    const renderMarkdown = vi.fn((source: string) => `<p data-markdown>${source}</p>`);
    const wrapper = mount(ReaderPane, {
      props: {
        segments: [segment("1.1", "paragraph"), segment("2.1", "section")],
        viewportAnchor: null,
        selectedLid: null,
        renderSeg: (value) => value.text,
        renderMarkdown,
        markdownHeadingLevel: () => null,
        isAsset: () => false,
        isHighlighted: () => false,
        highlightsOf: () => [],
        highlightCardsOf: () => [],
        visibleNotes: [short, { ...short, mem_id: "note-second" }, long],
        hlExcerpt: () => "",
        imageMeta: () => null,
        imageAsset: () => null,
      },
    });

    expect(wrapper.find('.prose .note-card').exists()).toBe(false);
    expect(wrapper.find('.prose .block-actions').exists()).toBe(false);
    expect(wrapper.findAll('.annotation-marker')).toHaveLength(2);
    expect(wrapper.findAll('.annotation-marker')[0].text()).toContain('2');
    await wrapper.findAll('.annotation-marker')[0].trigger('click');
    let preview = document.querySelector('.annotation-preview')!;
    expect(preview.textContent).toContain('Short **body**');
    expect(document.querySelectorAll('.annotation-preview')).toHaveLength(1);
    (preview.querySelector('footer button') as HTMLButtonElement).click();
    expect(wrapper.emitted('edit-note')?.at(-1)).toEqual([short]);
    await wrapper.vm.$nextTick();
    expect(document.querySelector('.annotation-preview')).toBeNull();
    await wrapper.findAll('.annotation-marker')[1].trigger('click');
    preview = document.querySelector('.annotation-preview')!;
    expect(preview.textContent).toContain('x'.repeat(400));
    (preview.querySelector('footer button:nth-child(2)') as HTMLButtonElement).click();
    expect(wrapper.emitted('delete-note')?.at(-1)).toEqual([long]);
    await wrapper.setProps({ visibleNotes: [short] });
    expect(document.querySelector('.annotation-preview')).toBeNull();
    await wrapper.findAll('.annotation-marker')[0].trigger('click');
    await wrapper.setProps({ contextKey: 'other-user' });
    expect(document.querySelector('.annotation-preview')).toBeNull();
    await wrapper.findAll('.annotation-marker')[0].trigger('click');
    await wrapper.setProps({ segments: [] });
    expect(document.querySelector('.annotation-preview')).toBeNull();

    wrapper.unmount();
  });

  it("scrolls an explicit outline navigation target to the pane top", async () => {
    const wrapper = mount(ReaderPane, {
      props: {
        segments: [
          segment("1.1", "paragraph"),
          segment("1.2", "paragraph"),
          segment("1.3", "paragraph"),
        ],
        viewportAnchor: "1.1",
        selectedLid: "1.1",
        renderSeg: (value) => value.text,
        renderMarkdown: (source) => source,
        markdownHeadingLevel: () => null,
        isAsset: () => false,
        isHighlighted: () => false,
        highlightsOf: () => [],
        highlightCardsOf: () => [],
        visibleNotes: [],
        hlExcerpt: () => "",
        imageMeta: () => null,
        imageAsset: () => null,
      },
    });
    const pane = wrapper.get(".reader-pane").element as HTMLElement;
    const target = wrapper.get('[data-lid="1.2"]').element as HTMLElement;
    pane.scrollTop = 180;
    vi.spyOn(pane, "getBoundingClientRect").mockReturnValue({
      x: 0, y: 100, top: 100, bottom: 500, left: 0, right: 600,
      width: 600, height: 400, toJSON: () => ({}),
    });
    vi.spyOn(target, "getBoundingClientRect").mockReturnValue({
      x: 0, y: 340, top: 340, bottom: 380, left: 0, right: 600,
      width: 600, height: 40, toJSON: () => ({}),
    });

    const exposed = wrapper.vm as unknown as {
      scrollLidIntoView?: (lid: string) => Promise<boolean>;
    };
    expect(exposed.scrollLidIntoView).toBeTypeOf("function");
    await exposed.scrollLidIntoView!("1.2");
    expect(pane.scrollTop).toBe(420);

    const appSource = readFileSync("src/App.vue", "utf8");
    expect(appSource).toContain("await readerPaneRef.value?.scrollLidIntoView(navigationTargetLid)");
    await wrapper.get(".reader-pane").trigger("wheel", { deltaY: 40 });
    expect(wrapper.emitted("viewport-interaction")).toHaveLength(1);
    wrapper.unmount();
  });

  it("emits placement input only from Pointer Events while placement is active", async () => {
    const wrapper = mount(ReaderPane, {
      props: {
        segments: [segment("1.1", "paragraph")],
        viewportAnchor: null,
        selectedLid: null,
        notePlacementActive: false,
        renderSeg: (value) => value.text,
        renderMarkdown: (source) => source,
        markdownHeadingLevel: () => null,
        isAsset: () => false,
        isHighlighted: () => false,
        highlightsOf: () => [],
        highlightCardsOf: () => [],
        visibleNotes: [],
        hlExcerpt: () => "",
        imageMeta: () => null,
        imageAsset: () => null,
      },
    });

    await wrapper.get('[data-lid="1.1"]').trigger("pointerup");
    expect(wrapper.emitted("note-placement-pointer")).toBeUndefined();

    await wrapper.setProps({ notePlacementActive: true });
    await wrapper.get('[data-lid="1.1"]').trigger("pointerup");
    expect(wrapper.emitted("note-placement-pointer")).toHaveLength(1);
    expect(wrapper.emitted("note-placement-target")?.at(-1)).toEqual([{ lid: "1.1" }]);
    wrapper.unmount();
  });

  it("previews and submits a real target at 390px while rejecting annotation controls", async () => {
    Object.defineProperty(window, "innerWidth", { value: 390, configurable: true });
    const wrapper = mount(ReaderPane, {
      props: {
        segments: [segment("1.1", "paragraph")],
        viewportAnchor: null,
        selectedLid: "1.1",
        notePlacementActive: true,
        renderSeg: (value) => value.text,
        renderMarkdown: (source) => source,
        markdownHeadingLevel: () => null,
        isAsset: () => false,
        isHighlighted: () => false,
        highlightsOf: () => [],
        highlightCardsOf: () => [],
        visibleNotes: [note("existing-note", "1.1", "Saved annotation")],
        hlExcerpt: () => "",
        imageMeta: () => null,
        imageAsset: () => null,
      },
    });
    const body = wrapper.get('[data-lid="1.1"]');

    await body.trigger("pointermove", { pointerType: "touch" });
    expect(body.classes()).toContain("note-placement-candidate");
    await body.trigger("pointerup", { pointerType: "touch" });
    expect(wrapper.emitted("note-placement-target")?.at(-1)).toEqual([{ lid: "1.1" }]);

    expect(wrapper.find('[aria-label="段落操作"]').exists()).toBe(false);
    const action = wrapper.get(".annotation-marker");
    await action.trigger("pointermove", { pointerType: "touch" });
    expect(body.classes()).not.toContain("note-placement-candidate");
    await action.trigger("pointerup", { pointerType: "touch" });
    expect(wrapper.emitted("note-placement-invalid")).toHaveLength(1);
    wrapper.unmount();
  });

  it("keeps wide code local, toggles soft wrap, and expands without changing source text", async () => {
    const wrapper = mount(ReaderPane, {
      props: {
        segments: [segment("code.1", "code")],
        viewportAnchor: null,
        selectedLid: null,
        renderSeg: (value) => value.text,
        renderMarkdown: (source) => source,
        markdownHeadingLevel: () => null,
        isAsset: () => true,
        isHighlighted: () => false,
        highlightsOf: () => [],
        highlightCardsOf: () => [],
        visibleNotes: [],
        hlExcerpt: () => "",
        imageMeta: () => null,
        imageAsset: () => null,
      },
    });
    const original = wrapper.get(".asset-source.asset-code").text();
    const actions = wrapper.findAll(".asset-head button");
    expect(actions.map(action => action.text())).toEqual(["换行", "展开"]);
    await wrapper.get("code").trigger("click");
    expect(wrapper.emitted("select")).toBeUndefined();
    await wrapper.setProps({ selectedLid: "code.1" });
    expect(wrapper.get(".asset-block").classes()).not.toContain("selected");
    await actions[0].trigger("click");
    expect(wrapper.get(".asset-source.asset-code").classes()).toContain("soft-wrap");
    expect(wrapper.get(".asset-source.asset-code").text()).toBe(original);
    await actions[1].trigger("click");
    expect(wrapper.get(".asset-block").classes()).toContain("asset-expanded");
    await wrapper.findAll(".asset-head button")[1].trigger("click");
    expect(wrapper.get(".asset-block").classes()).not.toContain("asset-expanded");
    wrapper.unmount();
  });

  it("renders an EPUB table with wrap and expand without selecting the block on click", async () => {
    const table = { ...segment("table.1", "table"), text: "| 状态 | 归属 |\n| 笔记 | 读者 |" };
    const wrapper = mount(ReaderPane, {
      props: {
        segments: [table], viewportAnchor: null, selectedLid: null,
        renderSeg: (value) => renderTableSource(value.text),
        renderMarkdown: (source) => source, markdownHeadingLevel: () => null,
        isAsset: () => true, isHighlighted: () => false,
        highlightsOf: () => [], highlightCardsOf: () => [], visibleNotes: [],
        hlExcerpt: () => "", imageMeta: () => null, imageAsset: () => null,
      },
    });
    expect(wrapper.get(".asset-source.asset-table").element.tagName).toBe("DIV");
    expect(wrapper.findAll("table tr")).toHaveLength(2);
    const original = wrapper.get("table").html();
    const actions = wrapper.findAll(".asset-head button");
    expect(actions.map(action => action.text())).toEqual(["换行", "展开"]);
    await actions[0].trigger("click");
    expect(actions[0].attributes("aria-pressed")).toBe("true");
    expect(wrapper.get(".asset-source.asset-table").classes()).toContain("soft-wrap");
    await actions[1].trigger("click");
    expect(wrapper.get(".asset-block").classes()).toContain("asset-expanded");
    await wrapper.get("td").trigger("click");
    expect(wrapper.emitted("select")).toBeUndefined();
    await wrapper.setProps({ selectedLid: "table.1" });
    expect(wrapper.get(".asset-block").classes()).not.toContain("selected");
    expect(wrapper.get("table").html()).toBe(original);
    expect(table.text).toBe("| 状态 | 归属 |\n| 笔记 | 读者 |");
    wrapper.unmount();
  });
});
