// @vitest-environment happy-dom
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import QuestionImageInput from './QuestionImageInput.vue';
import { captureQuestionImage, scoreImageRect } from '../question-image';
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });
function canvas() {
  const drawImage = vi.fn();
  vi.spyOn(HTMLCanvasElement.prototype,'getContext').mockReturnValue({ drawImage } as unknown as CanvasRenderingContext2D);
  vi.spyOn(HTMLCanvasElement.prototype,'toDataURL').mockReturnValue('data:image/png;base64,cGl4ZWxz');
  return drawImage;
}
it('selects any score region in either drag direction, clips edges, and ignores clicks', () => {
  const box = { left: 20, top: 100, width: 1000, height: 500 };
  expect(scoreImageRect(box, { x: 520, y: 350 }, { x: -10, y: 50 })).toEqual([0, 0, .5, .5]);
  expect(scoreImageRect(box, { x: 20, y: 100 }, { x: 1020, y: 600 })).toEqual([0, 0, 1, 1]);
  expect(scoreImageRect(box, { x: 50, y: 150 }, { x: 51, y: 151 })).toBeNull();
});
it('captures a right-edge crop directly from the displayed video without seeking', () => {
  const draw = canvas(), video = document.createElement('video');
  Object.defineProperties(video,{ readyState:{value:2},videoWidth:{value:1920},videoHeight:{value:1080} });
  const image = captureQuestionImage(video,{frame:{video:{media_id:'v',media_revision:'1'},actual_pts_ms:15466.6875,width:1920,height:1080},
    rect_normalized:[113/1920,18/1080,1807/1920,1050/1080]});
  expect(draw).toHaveBeenCalledWith(video,113,18,1807,1050,0,0,1807,1050);
  expect(image).toEqual({png_base64:'cGl4ZWxz',width:1807,height:1050,origin:'selection'});
  expect(video.currentTime).toBe(0);
});
it('accepts a pasted picture, keeps ordinary text paste native, and removes the preview', async () => {
  canvas(); const close = vi.fn();
  vi.stubGlobal('createImageBitmap',vi.fn(async()=>({width:1920,height:1080,close})));
  const wrapper = mount(QuestionImageInput);
  const preventDefault = vi.fn();
  wrapper.vm.paste({clipboardData:{items:[]},preventDefault} as unknown as ClipboardEvent);
  expect(preventDefault).not.toHaveBeenCalled();
  const file = new File(['png'],'capture.png',{type:'image/png'});
  wrapper.vm.paste({clipboardData:{items:[{kind:'file',type:'image/png',getAsFile:()=>file}]},preventDefault} as unknown as ClipboardEvent);
  await flushPromises();
  expect(preventDefault).toHaveBeenCalledOnce(); expect(close).toHaveBeenCalledOnce();
  expect(wrapper.emitted('loading')).toEqual([[true],[false]]);
  const image = wrapper.emitted('change')![0][0] as never;
  await wrapper.setProps({image});
  expect(wrapper.get('img[alt="待发送的问题图片"]').attributes('src')).toBe('data:image/png;base64,cGl4ZWxz');
  await wrapper.get('button[aria-label="移除图片"]').trigger('click');
  expect(wrapper.emitted('change')![1]).toEqual([null]); wrapper.unmount();
});
it('reports an unreadable upload without discarding an existing attachment', async () => {
  const image = {png_base64:'old',width:2,height:1,origin:'upload' as const};
  vi.stubGlobal('createImageBitmap',vi.fn(async()=>{throw new Error('decode');}));
  const wrapper = mount(QuestionImageInput,{props:{image}});
  const file = new File(['broken'],'broken.png',{type:'image/png'});
  const picker = wrapper.get('input[type=file]');
  Object.defineProperty(picker.element,'files',{value:[file]}); await picker.trigger('change'); await flushPromises();
  expect(wrapper.get('[role=alert]').text()).toContain('无法读取'); expect(wrapper.emitted('change')).toBeUndefined();
  expect(wrapper.get('img').attributes('src')).toBe('data:image/png;base64,old'); wrapper.unmount();
});
