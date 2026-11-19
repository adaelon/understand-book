// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import AgentActivities from "./AgentActivities.vue";
import type { RunActivity } from "../agent-run-state";

const activity = (step_id: number, label: string, status: RunActivity["status"] = "running"): RunActivity => ({
  step_id, label, status, parent_step_id: null, kind: "tool", name: "book.read", started_ms: 0,
  duration_ms: status === "running" ? null : 1900, result_count: null, error_code: null,
  usage_total_tokens: null, usage: null, model_first_text_ms: null, model_name: null,
  model_name_source: null, accepted_evidence_count: null, evidence_refs: [],
});
const commentary = (id: number, text: string): RunActivity => ({ ...activity(id, text, 'succeeded'), kind: 'commentary', name: 'agent.commentary' });

describe("AgentActivities work feed", () => {
  it("shows progress paragraphs and top-level tools in order while keeping model internals out of the feed", () => {
    const wrapper = mount(AgentActivities, { props: { active: true, activities: [
      { ...activity(1, '生成回答', 'succeeded'), kind: 'model' },
      commentary(2, '我会先读取 2.1 节，再把架构关系整理成演示。'),
      { ...activity(3, '发布进展', 'succeeded'), name: 'agent.progress' },
      activity(4, '读取原文', 'succeeded'),
      { ...activity(5, '子模型请求', 'succeeded'), kind: 'model', parent_step_id: 4 },
      commentary(6, '已经确认三个关键部分，接下来制作演示。'),
      activity(7, '制作演示'),
    ] } });
    expect(wrapper.findAll('li').map(row => row.attributes('data-step-id'))).toEqual(['2', '4', '6', '7']);
    expect(wrapper.get('.activity-heading').text()).toBe('正在工作 · 2 项操作');
    expect(wrapper.findAll('.activity-commentary').map(row => row.text())).toEqual([
      '我会先读取 2.1 节，再把架构关系整理成演示。', '已经确认三个关键部分，接下来制作演示。',
    ]);
    expect(wrapper.find('.activity-commentary .activity-status').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('生成回答');
    expect(wrapper.text()).not.toContain('秒');
    expect(wrapper.find('button').exists()).toBe(false);
    wrapper.unmount();
  });

  it("collapses completed work, restores the complete public history, and retains the expansion choice", async () => {
    const first = commentary(1, '我会先确认原文中的架构。');
    const second = activity(2, '读取原文', 'succeeded');
    const wrapper = mount(AgentActivities, { props: { active: true, activities: [first, second] } });
    expect(wrapper.find('ol').exists()).toBe(true);
    await wrapper.setProps({ active: false });
    const summary = wrapper.get('button');
    expect(summary.text()).toBe('查看工作过程 · 1 项操作');
    expect(wrapper.find('ol').exists()).toBe(false);
    expect(summary.attributes('aria-expanded')).toBe('false');
    await summary.trigger('click');
    expect(summary.attributes('aria-expanded')).toBe('true');
    expect(wrapper.get('ol').attributes('id')).toBe(summary.attributes('aria-controls'));
    expect(wrapper.findAll('li').map(row => row.attributes('data-step-id'))).toEqual(['1', '2']);
    await wrapper.setProps({ activities: [first, second, activity(3, '制作演示', 'succeeded')] });
    expect(wrapper.findAll('li')).toHaveLength(3);
    expect(summary.attributes('aria-expanded')).toBe('true');
    await summary.trigger('click');
    expect(wrapper.find('ol').exists()).toBe(false);
    wrapper.unmount();
  });

  it("distinguishes failed, empty, rejected and cancelled actions without requiring diagnostics", () => {
    const wrapper = mount(AgentActivities, { props: { active: true, activities: [
      activity(1, '搜索原文', 'no_result'), activity(2, '制作演示', 'failed'),
      activity(3, '保存演示', 'rejected'), activity(4, '读取原文', 'cancelled'),
    ] } });
    expect(wrapper.findAll('.activity-status').map(row => row.text())).toEqual(['无结果', '失败', '未执行', '已停止']);
    wrapper.unmount();
  });

  it("keeps all diagnostic records, timing, errors, result counts and usage available", () => {
    const wrapper = mount(AgentActivities, { props: { diagnostic: true, activities: [
      { ...activity(1, '生成回答', 'succeeded'), kind: 'model', usage_total_tokens: 12, duration_ms: 0 },
      { ...activity(2, '读取原文', 'failed'), error_code: 'MODEL_ERROR', result_count: 0, duration_ms: 12 },
      { ...activity(3, '子模型请求', 'succeeded'), kind: 'model', parent_step_id: 2, duration_ms: 100 },
    ] } });
    expect(wrapper.find('button').exists()).toBe(false);
    expect(wrapper.findAll('li')).toHaveLength(3);
    expect(wrapper.findAll('details')).toHaveLength(3);
    expect(wrapper.findAll('li')[0]!.text()).toContain('<1 毫秒');
    expect(wrapper.findAll('li')[1]!.text()).toContain('12.0 毫秒');
    expect(wrapper.findAll('li')[2]!.text()).toContain('0.1 秒');
    expect(wrapper.findAll('li')[2]!.text()).toContain('所属步骤 2');
    expect(wrapper.text()).toContain('MODEL_ERROR');
    expect(wrapper.text()).toContain('0 项结果');
    expect(wrapper.text()).toContain('已记录模型用量 12 tokens');
    wrapper.unmount();
  });

  it("shows one working indicator before public progress arrives and no empty completed disclosure", async () => {
    const wrapper = mount(AgentActivities, { props: { active: true, activities: [
      { ...activity(1, '生成回答'), kind: 'model' },
    ] } });
    expect(wrapper.text()).toBe('正在工作');
    expect(wrapper.find('li').exists()).toBe(false);
    await wrapper.setProps({ active: false });
    expect(wrapper.find('.activity-group').exists()).toBe(false);
    await wrapper.setProps({ activities: [] });
    expect(wrapper.find('.activity-group').exists()).toBe(false);
    wrapper.unmount();
  });
});
