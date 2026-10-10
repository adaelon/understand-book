// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import ShareImagePanel from './ShareImagePanel.vue';
import { answerShareSource, renderShare, saveShare } from '../reading-share';
vi.mock('../reading-share', async original => ({ ...await original<typeof import('../reading-share')>(), renderShare: vi.fn(), saveShare: vi.fn() }));
const source = { parts: [{ id: 'body' as const, label: '阅读笔记', text: '保留的原笔记' }], sources: ['无已记录出处'], association: '' };
let wrapper: ReturnType<typeof mount>;
beforeEach(() => {
  vi.spyOn(HTMLDialogElement.prototype, 'showModal').mockImplementation(() => {});
  vi.spyOn(HTMLDialogElement.prototype, 'close').mockImplementation(() => {});
  vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:preview');
  vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
  vi.mocked(renderShare).mockReset(); vi.mocked(saveShare).mockReset();
  wrapper = mount(ShareImagePanel, { props: { source } });
});
afterEach(() => { wrapper.unmount(); vi.restoreAllMocks(); });
const button = (name: string) => wrapper.findAll('button').find(b => b.text() === name)!;
it('RS6 adjusts a frozen answer excerpt while preserving edits and invalidating its preview', async () => {
  wrapper.unmount();
  const full = '仅当条件成立时，结论才成立。另一段解释。';
  const answer = answerShareSource(full, '结论才成立。', 'chat', 'turn', ['原材料 · 第 2 页']);
  wrapper = mount(ShareImagePanel, { props: { source: answer } });
  expect(wrapper.find('option[value="excerpt"]').exists()).toBe(false);
  await wrapper.get('input:not([type=checkbox])').setValue('我的标题');
  await wrapper.get('textarea').setValue('我的感想');
  vi.mocked(renderShare).mockResolvedValue([new Blob(['png'])]);
  await button('预览图片').trigger('click'); await flushPromises();
  expect(renderShare).toHaveBeenLastCalledWith(expect.objectContaining({ layout: 'understanding' }));
  await button('调整回答范围').trigger('click');
  await button('使用选中文字').trigger('click');
  expect(wrapper.text()).toContain('请先在回答中选择连续文字');
  const field = wrapper.get('[aria-label="原回答文字"]').element as HTMLTextAreaElement;
  field.setSelectionRange(0, full.indexOf('另一段'));
  await button('使用选中文字').trigger('click');
  expect(wrapper.find('img').exists()).toBe(false);
  await button('预览图片').trigger('click'); await flushPromises();
  expect(renderShare).toHaveBeenLastCalledWith(expect.objectContaining({ title: '我的标题', reflection: '我的感想',
    source: expect.objectContaining({ parts: [expect.objectContaining({ text: '仅当条件成立时，结论才成立。' })] }) }));
  await button('调整回答范围').trigger('click'); await button('包含全部回答文字').trigger('click');
  await button('预览图片').trigger('click'); await flushPromises();
  expect(renderShare).toHaveBeenLastCalledWith(expect.objectContaining({ source: expect.objectContaining({
    association: '回答文字全文 · 本回答来源', parts: [expect.objectContaining({ text: full })] }) }));
  expect(answer.parts[0].text).toBe('结论才成立。');
});
it('retains edits after render/save failure and downloads exactly the latest preview blob', async () => {
  const png = new Blob(['png'], { type: 'image/png' });
  await wrapper.get('input:not([type=checkbox])').setValue('我的标题');
  await wrapper.get('textarea').setValue('这次的感想');
  vi.mocked(renderShare).mockRejectedValueOnce(new Error('字体失败')).mockResolvedValue([png]);
  await button('预览图片').trigger('click'); await flushPromises();
  expect(wrapper.get('[role=alert]').text()).toContain('字体失败');
  expect((wrapper.get('textarea').element as HTMLTextAreaElement).value).toBe('这次的感想');
  await button('预览图片').trigger('click'); await flushPromises();
  expect(wrapper.get('img').attributes('src')).toBe('blob:preview');
  vi.mocked(saveShare).mockRejectedValueOnce(new Error('disk')).mockResolvedValue(true);
  await button('下载 PNG').trigger('click'); await flushPromises(); expect(wrapper.text()).toContain('保存失败');
  await button('下载 PNG').trigger('click'); await flushPromises();
  expect(saveShare).toHaveBeenLastCalledWith(png, 'blob:preview', expect.any(Function), undefined);
  expect(renderShare).toHaveBeenLastCalledWith(expect.objectContaining({ title: '我的标题', reflection: '这次的感想' }));
  expect(source.parts[0].text).toBe('保留的原笔记');
});
it('invalidates preview immediately after an edit and ignores late render after unmount', async () => {
  let resolve!: (blob: Blob[]) => void;
  vi.mocked(renderShare).mockImplementation(() => new Promise(yes => { resolve = yes; }));
  await button('预览图片').trigger('click');
  await wrapper.get('textarea').setValue('新感想');
  resolve([new Blob(['old'])]); await flushPromises(); expect(wrapper.find('img').exists()).toBe(false);
  await button('预览图片').trigger('click'); wrapper.unmount();
  resolve([new Blob(['late'])]); await flushPromises(); expect(URL.createObjectURL).not.toHaveBeenCalled();
});

it('previews and saves each generated page with a distinct filename, then invalidates all pages', async () => {
  const blobs = [new Blob(['first']), new Blob(['second'])];
  vi.mocked(renderShare).mockResolvedValue(blobs);
  vi.mocked(URL.createObjectURL).mockReturnValueOnce('blob:first').mockReturnValueOnce('blob:second');
  await button('预览图片').trigger('click'); await flushPromises();
  expect(wrapper.get('img').attributes('src')).toBe('blob:first');
  await button('下一页').trigger('click');
  expect(wrapper.get('img').attributes('src')).toBe('blob:second');
  await button('下载 PNG').trigger('click'); await flushPromises();
  expect(saveShare).toHaveBeenLastCalledWith(blobs[1], 'blob:second', expect.any(Function), 2);
  await wrapper.get('textarea').setValue('修改后');
  expect(wrapper.find('img').exists()).toBe(false);
  expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:first');
  expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:second');
});

it('keeps a cancelled save retryable and invalidates an open save when leaving the scene', async () => {
  vi.mocked(renderShare).mockResolvedValue([new Blob(['png'])]);
  await button('预览图片').trigger('click'); await flushPromises();
  vi.mocked(saveShare).mockResolvedValueOnce(false);
  await button('下载 PNG').trigger('click'); await flushPromises();
  expect(wrapper.find('[role=alert]').exists()).toBe(false);
  expect(wrapper.find('[role=status]').exists()).toBe(false);
  expect(wrapper.get('img').attributes('src')).toBe('blob:preview');
  let finish!: (saved: boolean) => void;
  vi.mocked(saveShare).mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  await button('下载 PNG').trigger('click');
  const current = vi.mocked(saveShare).mock.calls.at(-1)![2];
  expect(current()).toBe(true);
  wrapper.unmount();
  expect(current()).toBe(false);
  expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:preview');
  finish(false); await flushPromises();
});


it('RS7 fixes scene context and changes orientation without recapturing the diagram', async () => {
  wrapper.unmount();
  const diagram = { ...source, diagram: { png: 'data:image/png;base64,AA==', width: 640, height: 360, title: '当前图解' } };
  wrapper = mount(ShareImagePanel, { props: { source: diagram } });
  expect(wrapper.get('input[type=checkbox]').attributes('disabled')).toBeDefined();
  expect(wrapper.find('option[value="excerpt"]').exists()).toBe(false);
  vi.mocked(renderShare).mockResolvedValue([new Blob(['png'])]);
  await button('预览图片').trigger('click'); await flushPromises();
  expect(renderShare).toHaveBeenLastCalledWith(expect.objectContaining({ orientation: 'landscape', layout: 'diagram' }));
  await wrapper.findAll('select').find(select => select.find('option[value="portrait"]').exists())!.setValue('portrait');
  expect(wrapper.find('img').exists()).toBe(false);
  await button('预览图片').trigger('click'); await flushPromises();
  expect(renderShare).toHaveBeenLastCalledWith(expect.objectContaining({ orientation: 'portrait', source: diagram }));
  vi.mocked(renderShare).mockRejectedValueOnce(new Error('图解过长'));
  await button('预览图片').trigger('click'); await flushPromises();
  await button('返回图解').trigger('click');
  expect(wrapper.emitted('close')).toHaveLength(1);
});
