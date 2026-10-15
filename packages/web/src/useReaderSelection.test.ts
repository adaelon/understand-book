// @vitest-environment happy-dom
import { nextTick, ref } from "vue";
import { mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { describe, expect, it, vi } from "vitest";
import { readReaderSelection, useReaderSelection } from "./useReaderSelection";

function selectText(node: Text, start: number, end: number) {
  const range = document.createRange();
  range.setStart(node, start);
  range.setEnd(node, end);
  Object.defineProperty(range, "getBoundingClientRect", {
    value: () => ({ left: 10, top: 20, width: 30, height: 12 }),
  });
  const selection = window.getSelection()!;
  selection.removeAllRanges();
  selection.addRange(range);
  return range;
}

describe("reader selection", () => {
  it('accepts both columns of one current screen and excludes prepared or animated screens', () => {
    const root = document.createElement('div');
    root.innerHTML = '<div class="focus-screen"><article><span data-lid="1" data-source-start="0">left</span></article><article><span data-lid="1" data-source-start="4">right</span></article></div><div class="focus-screen" hidden inert><span data-lid="2">prefetch</span></div>';
    document.body.append(root);
    const nodes = root.querySelectorAll('span'), first = nodes[0].firstChild!, last = nodes[1].firstChild!;
    const range = selectText(first as Text, 1, 3); range.setEnd(last, 3);
    window.getSelection()!.removeAllRanges(); window.getSelection()!.addRange(range);
    expect(readReaderSelection(root)?.text).toBe('eftrig');
    nodes[0].closest('.focus-screen')!.setAttribute('inert', '');
    expect(readReaderSelection(root)).toBeNull();
    selectText(nodes[2].firstChild as Text, 0, 3);
    expect(readReaderSelection(root)).toBeNull();
    root.remove(); window.getSelection()!.removeAllRanges();
  });
  it("clones an in-reader range and rejects editable or crossing selections", () => {
    const root = document.createElement("div");
    root.innerHTML = '<p data-lid="L1">alpha😀beta</p><textarea>draft</textarea>';
    document.body.append(root);
    const text = root.querySelector("p")!.firstChild as Text;
    const range = selectText(text, 5, 7);
    const snapshot = readReaderSelection(root);
    expect(snapshot?.text).toBe("😀");
    expect(snapshot?.range).not.toBe(range);
    const inputText = root.querySelector("textarea")!.firstChild as Text;
    selectText(inputText, 0, 2);
    expect(readReaderSelection(root)).toBeNull();
    root.remove();
  });

  it("reacts to selectionchange without mouseup, dedupes, and preserves a frozen selection on outside focus", async () => {
    const selected = vi.fn();
    const Component = defineComponent({
      setup() {
        const root = ref<HTMLElement | null>(null);
        useReaderSelection(root, selected);
        return () => h("div", { ref: root }, [h("p", { "data-lid": "L1" }, "touch text")]);
      },
    });
    const wrapper = mount(Component, { attachTo: document.body });
    const text = wrapper.find("p").element.firstChild as Text;
    selectText(text, 0, 5);
    document.dispatchEvent(new Event("selectionchange"));
    await nextTick();
    expect(selected).toHaveBeenCalledTimes(1);
    document.dispatchEvent(new Event("selectionchange"));
    await nextTick();
    expect(selected).toHaveBeenCalledTimes(1);
    window.getSelection()!.removeAllRanges();
    document.body.dispatchEvent(new Event("selectionchange"));
    await nextTick();
    expect(selected).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });
});
