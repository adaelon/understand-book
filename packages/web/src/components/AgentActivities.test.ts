// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import AgentActivities from "./AgentActivities.vue";
import type { RunActivity } from "../agent-run-state";

const activity = (step_id: number, label: string, status: RunActivity["status"] = "running"): RunActivity => ({
  step_id, label, status, parent_step_id: null, kind: "model", name: "outer", started_ms: 0,
  duration_ms: status === "running" ? null : 1900, result_count: null, error_code: null,
  usage_total_tokens: null, usage: null, model_first_text_ms: null, model_name: null,
  model_name_source: null, accepted_evidence_count: null, evidence_refs: [],
});

describe("AgentActivities disclosure", () => {
  it("omits sub-100ms timings in the summary and list while retaining second-scale durations", async () => {
    const wrapper = mount(AgentActivities, { props: { activities: [
      { ...activity(1, "生成回答", "succeeded"), duration_ms: 1900 },
      { ...activity(2, "读取偏好", "succeeded"), duration_ms: 12 },
    ] } });
    expect(wrapper.get('button').text()).not.toContain('秒');
    await wrapper.get('button').trigger('click');
    expect(wrapper.findAll('li')[0]!.text()).toContain('1.9 秒');
    expect(wrapper.findAll('li')[1]!.text()).not.toContain('秒');
    expect(wrapper.text()).not.toContain('0.0 秒');
    wrapper.unmount();
  });

  it("shows short diagnostic durations in milliseconds including sub-millisecond work", () => {
    const wrapper = mount(AgentActivities, { props: { diagnostic: true, activities: [
      { ...activity(1, "读取偏好", "succeeded"), duration_ms: 0 },
      { ...activity(2, "读取原文", "succeeded"), duration_ms: 12 },
      { ...activity(3, "解析", "succeeded"), duration_ms: 100 },
    ] } });
    expect(wrapper.findAll('li')[0]!.text()).toContain('<1 毫秒');
    expect(wrapper.findAll('li')[1]!.text()).toContain('12.0 毫秒');
    expect(wrapper.findAll('li')[2]!.text()).toContain('0.1 秒');
    expect(wrapper.text()).not.toContain('0.0 秒');
    wrapper.unmount();
  });

  it("shows only the latest action, expands the complete ordered history, and collapses again", async () => {
    const wrapper = mount(AgentActivities, { props: { activities: [activity(1, "处理阅读偏好", "succeeded"), activity(2, "生成回答")] } });
    const summary = wrapper.get("button");
    expect(summary.text()).toContain("生成回答");
    expect(summary.text()).toContain("进行中");
    expect(wrapper.text()).not.toContain("处理阅读偏好");
    expect(summary.attributes("aria-expanded")).toBe("false");
    await summary.trigger("click");
    expect(summary.attributes("aria-expanded")).toBe("true");
    expect(wrapper.get("ol").attributes("id")).toBe(summary.attributes("aria-controls"));
    expect(wrapper.findAll("li .activity-label").map(row => row.text())).toEqual(["处理阅读偏好", "生成回答"]);
    expect(wrapper.get(".activity-chevron").classes()).toContain("expanded");
    await summary.trigger("click");
    expect(wrapper.find("ol").exists()).toBe(false);
    wrapper.unmount();
  });

  it("updates the summary and history without resetting the reader's expansion choice", async () => {
    const first = activity(1, "处理阅读偏好", "succeeded");
    const wrapper = mount(AgentActivities, { props: { activities: [first] } });
    const next = activity(2, "生成回答");
    await wrapper.setProps({ activities: [first, next] });
    expect(wrapper.get("button").text()).toContain("生成回答");
    expect(wrapper.find("ol").exists()).toBe(false);
    await wrapper.get("button").trigger("click");
    await wrapper.setProps({ activities: [first, { ...next, status: "succeeded", duration_ms: 12100 }, activity(3, "读取原文")] });
    expect(wrapper.get("button").text()).toContain("读取原文");
    expect(wrapper.findAll("li")).toHaveLength(3);
    expect(wrapper.findAll("li")[1]!.text()).toContain("12.1 秒");
    expect(wrapper.get("button").attributes("aria-expanded")).toBe("true");
    wrapper.unmount();
  });

  it("keeps diagnostic records, results, errors and usage available without an outer toggle", () => {
    const wrapper = mount(AgentActivities, { props: { diagnostic: true, activities: [
      { ...activity(1, "处理阅读偏好", "succeeded"), usage_total_tokens: 12 },
      { ...activity(2, "生成回答", "failed"), error_code: "MODEL_ERROR", result_count: 0 },
    ] } });
    expect(wrapper.find("button").exists()).toBe(false);
    expect(wrapper.findAll("li")).toHaveLength(2);
    expect(wrapper.findAll("details")).toHaveLength(2);
    expect(wrapper.text()).toContain("MODEL_ERROR");
    expect(wrapper.text()).toContain("0 项结果");
    expect(wrapper.text()).toContain("已记录模型用量 12 tokens");
    wrapper.unmount();
  });

  it("renders no activity control before the first action", () => {
    const wrapper = mount(AgentActivities, { props: { activities: [] } });
    expect(wrapper.find(".activity-group").exists()).toBe(false);
    wrapper.unmount();
  });
});
