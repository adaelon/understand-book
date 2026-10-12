// @vitest-environment happy-dom
import { flushPromises, mount } from "@vue/test-utils";
import { expect, it, vi } from "vitest";
import { api } from "../api";
import AgentPresentation from "./AgentPresentation.vue";

it("keeps the live frame when a parent renders the same version reference again", async () => {
  const read = vi.spyOn(api, "presentationRead").mockResolvedValue({
    animation_assets: {},
    reference: { presentation_id: "p", revision: 1 }, title: "Experiment", entrypoint: "index.html",
    content_files: { "index.html": "<p>Experiment</p>" }, sources: [], assumptions: [], initial_state: {},
    restored_state: null, restored_state_revision: null, readable_view: { parts: [], sources: [] },
  });
  const wrapper = mount(AgentPresentation, { props: { sessionId: "s", turnId: "t", reference: { presentation_id: "p", revision: 1 } } });
  try {
    await flushPromises();
    const original = wrapper.find("iframe").element;
    await wrapper.setProps({ busy: true, reference: { presentation_id: "p", revision: 1 } });
    await flushPromises();
    expect(read).toHaveBeenCalledTimes(1);
    expect(wrapper.find("iframe").element).toBe(original);
    await wrapper.find("iframe").trigger("load");
    for (let index = 0; index < 10; index += 1) {
      await wrapper.get(".presentation-expand").trigger("click");
    }
    expect(wrapper.find("iframe").element).toBe(original);
    expect(wrapper.find("iframe").attributes("data-load-count")).toBe("1");
    await wrapper.setProps({ reference: { presentation_id: "p", revision: 2 } });
    await flushPromises();
    expect(read).toHaveBeenCalledTimes(2);
  } finally { wrapper.unmount(); read.mockRestore(); }
});

it("zooms and opens readable content without rebuilding the live presentation", async () => {
  const read = vi.spyOn(api, "presentationRead").mockResolvedValue({
    animation_assets: {}, reference: { presentation_id: "p", revision: 1 }, title: "Experiment", entrypoint: "index.html",
    content_files: { "index.html": '<input id="parameter" value="7">' }, sources: [], assumptions: [], initial_state: {},
    restored_state: null, restored_state_revision: null, readable_view: { parts: [{ kind: "markdown", text: "**说明**" }], sources: [] },
  });
  const wrapper = mount(AgentPresentation, { props: { sessionId: "s", turnId: "t", reference: { presentation_id: "p", revision: 1 } } });
  try {
    await flushPromises();
    const original = wrapper.get("iframe").element;
    expect(wrapper.find(".presentation-readable").exists()).toBe(false);
    await wrapper.get('[aria-label="缩小演示"]').trigger("click");
    expect(wrapper.get('[aria-label="恢复演示原始大小"]').text()).toBe("90%");
    await wrapper.get('[aria-label="文字说明与来源"]').trigger("click");
    expect(wrapper.get(".readable-text strong").text()).toBe("说明");
    expect(wrapper.get("iframe").element).toBe(original);
    expect(read).toHaveBeenCalledTimes(1);
    await wrapper.get('[aria-label="恢复演示原始大小"]').trigger("click");
    expect(wrapper.get('[aria-label="恢复演示原始大小"]').text()).toBe("100%");
  } finally { wrapper.unmount(); read.mockRestore(); }
});
