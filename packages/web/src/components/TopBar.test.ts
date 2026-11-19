// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import TopBar from "./TopBar.vue";

const props = {
  chapterTitle: "第一章",
  progressPct: 42,
  anchorLid: null,
  debugOpen: false,
  leftRailOpen: false,
  buildIntentOpen: false,
  buildIntentAvailable: false,
  workbenchAvailable: false,
  desktopHost: false,
  mobileCollapsible: true,
  mobileOpen: false,
};

describe("TopBar", () => {
  it('offers model settings to a local browser and keeps them out of ordinary hosted readers', async () => {
    const wrapper = mount(TopBar, { props });
    expect(wrapper.findAll('button').some(button => button.text() === '模型设置')).toBe(false);
    await wrapper.setProps({ modelSettingsAvailable: true, mobileOpen: true });
    await wrapper.findAll('button').find(button => button.text() === '模型设置')!.trigger('click');
    expect(wrapper.emitted('open-settings')).toHaveLength(1);
    await wrapper.setProps({ desktopHost: true });
    expect(wrapper.findAll('button').some(button => button.text() === '打开桌面设置')).toBe(true);
    expect(wrapper.findAll('button').some(button => button.text() === '模型设置')).toBe(false);
    wrapper.unmount();
  });
  it('offers the board only when available and closes the mobile menu after opening it', async () => {
    const wrapper = mount(TopBar, { props: { ...props, mobileOpen: true } });
    expect(wrapper.findAll('button').some(button => button.text() === '当前资料的画板')).toBe(false);
    await wrapper.setProps({ boardAvailable: true });
    await wrapper.findAll('button').find(button => button.text() === '当前资料的画板')!.trigger('click');
    expect(wrapper.emitted('open-board')).toHaveLength(1);
    expect(wrapper.emitted('close-mobile')).toHaveLength(1);
    wrapper.unmount();
  });
  it('uses the current chapter to open the outline and keeps guided learning off the toolbar', async () => {
    const wrapper = mount(TopBar, { props: { ...props, materialTitle: '一本书的完整标题' } });
    const position = wrapper.get('.chapter-position');
    expect(position.get('.outline-label').text()).toBe('目录');
    expect(position.text()).toContain(props.chapterTitle);
    expect(position.text()).not.toContain('一本书的完整标题');
    await position.trigger('click');
    expect(wrapper.emitted('toggle-left-rail')).toHaveLength(1);
    await wrapper.get('.tools-toggle').trigger('click');
    expect(wrapper.text()).not.toContain('带我读懂');
  });
  it('keeps the material name visible for media without a book outline', () => {
    const wrapper = mount(TopBar, { props: { ...props, videoMode: true, materialTitle: '课程名称', chapterTitle: '视频伴读' } });
    expect(wrapper.get('.material-title').text()).toBe('课程名称');
    expect(wrapper.find('.outline-label').exists()).toBe(false);
  });
  it('collapses desktop controls, keeps an expand entry, and follows focus mode', async () => {
    const wrapper = mount(TopBar, { props });
    await wrapper.get('.tools-toggle').trigger('click');
    await wrapper.get('.topbar-collapse').trigger('click');
    expect(wrapper.get('.topbar').classes()).toContain('desktop-collapsed');
    await wrapper.get('.topbar-expand').trigger('click');
    expect(wrapper.get('.topbar').classes()).not.toContain('desktop-collapsed');
    await wrapper.setProps({ focusReading: true });
    expect(wrapper.get('.topbar').classes()).toContain('desktop-collapsed');
    await wrapper.setProps({ focusReading: false });
    expect(wrapper.get('.topbar').classes()).not.toContain('desktop-collapsed');
  });
  it('offers an explicit reversible focus action', async () => {
    const wrapper = mount(TopBar, { props: { ...props, focusAvailable: true } });
    await wrapper.get('.topbar-focus').trigger('click');
    expect(wrapper.emitted('toggle-focus')).toHaveLength(1);
    await wrapper.setProps({ focusReading: true });
    expect(wrapper.get('.topbar-focus').attributes('aria-label')).toBe('退出专注');
    expect(wrapper.get('.topbar-focus').attributes('aria-pressed')).toBe('true');
  });
  it("keeps mobile actions closed until requested and closes after an action", async () => {
    const wrapper = mount(TopBar, { props });
    expect(wrapper.get(".topbar").classes()).toContain("mobile-collapsible");
    expect(wrapper.get(".topbar").classes()).not.toContain("mobile-open");
    expect(wrapper.find(".topbar-mobile-backdrop").exists()).toBe(false);

    await wrapper.setProps({ mobileOpen: true });
    expect(wrapper.get(".topbar").classes()).toContain("mobile-open");
    expect(wrapper.find(".topbar-mobile-backdrop").exists()).toBe(true);
    await wrapper.findAll('.topbar-menu button').find(button => button.text() === '新对话')!.trigger("click");
    expect(wrapper.emitted("new-chat")).toHaveLength(1);
    expect(wrapper.emitted("close-mobile")).toHaveLength(1);
  });

  it("closes the mobile overlay from its backdrop", async () => {
    const wrapper = mount(TopBar, { props: { ...props, mobileOpen: true } });
    await wrapper.get(".topbar-mobile-backdrop").trigger("click");
    expect(wrapper.emitted("close-mobile")).toHaveLength(1);
  });

  it('keeps secondary actions in a disclosure and returns keyboard focus on Escape', async () => {
    const wrapper = mount(TopBar, { attachTo: document.body, props: { ...props, typographyAvailable: true },
      slots: { navigation: '<nav>学习资料 学习画板</nav>' } });
    try {
      const trigger = wrapper.get('.tools-toggle');
      expect(wrapper.get('.topbar-menu').isVisible()).toBe(false);
      expect(wrapper.text()).not.toContain('打开书');
      await trigger.trigger('click');
      expect(trigger.attributes('aria-expanded')).toBe('true');
      const settings = wrapper.get('[data-typography-trigger]');
      expect(document.activeElement).toBe(settings.element);
      await settings.trigger('keydown.esc');
      expect(trigger.attributes('aria-expanded')).toBe('false');
      expect(document.activeElement).toBe(trigger.element);
      await trigger.trigger('click');
      await settings.trigger('click');
      expect(wrapper.emitted('open-typography')).toHaveLength(1);
      expect(wrapper.get('.topbar-menu').isVisible()).toBe(false);
    } finally { wrapper.unmount(); }
  });
});
