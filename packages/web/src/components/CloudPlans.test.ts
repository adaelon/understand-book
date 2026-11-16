// @vitest-environment happy-dom
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, expect, it, vi } from 'vitest';
import { installIdentity, network } from '../network-context';
import type { AccountCloudPlans, CloudPlanProduct } from '../cloud-plans';
import CloudPlans from './CloudPlans.vue';

const product: CloudPlanProduct = { product_id: 'day-basic', title: '测试用冲刺套餐', tier: 'basic', duration: 'day1', revision: 1, enabled: true, test_only: false, amount_fen: 1250, basis: { version: 'fixture', basis_micro_cny: 1_000_000 }, grant_milli_percent: 150000, storage_bytes: 2 * 1024 ** 3, max_file_bytes: 1024 ** 2, retention_seconds: 30 * 86400, instructions: '联系运营核款', saleable: true, unavailable_reasons: [] };
const empty: AccountCloudPlans = { current: null, next: null, catalog: { items: [product] } };
const space = { used_bytes: 1024, reserved_bytes: 512, total_bytes: 1536, limit_bytes: null, max_file_bytes: null, available_bytes: 0, can_upload: false, over_limit: false, expires_at: null, retained_until: null };
const wrappers: ReturnType<typeof mount>[] = [];
afterEach(() => { wrappers.splice(0).forEach(w => w.unmount()); vi.unstubAllGlobals(); network.value = { ...network.value, identity: null, enabled: false }; });
function start() { installIdentity({ user_id: 'A', csrf_token: 'a' }); const wrapper = mount(CloudPlans); wrappers.push(wrapper); return wrapper; }
const button = (wrapper: ReturnType<typeof mount>, label: string) => wrapper.findAll('button').find(b => b.text() === label)!;

it('lets an account without a period select duration and a fully priced plan without treating selection as payment', async () => {
  const fetcher = vi.fn(async (url: string, init: RequestInit) => {
    expect(init.method).toBe('GET');
    return Response.json(url.endsWith('/storage') ? space : empty);
  });
  vi.stubGlobal('fetch', fetcher); const wrapper = start(); await flushPromises();
  expect(wrapper.text()).toContain('当前没有生效的云端套餐');
  expect(wrapper.text()).toContain('该时长的套餐尚未开放购买');
  await button(wrapper, '1 天冲刺').trigger('click');
  expect(wrapper.text()).toContain('¥12.50'); expect(wrapper.text()).toContain('基础版用量的 1 倍');
  expect(wrapper.text()).toContain('连续 24 小时'); expect(wrapper.text()).toContain('不自动扣款');
  await button(wrapper, '查看开通方式').trigger('click');
  expect(wrapper.get('[aria-label="套餐开通方式"]').text()).toContain('联系运营核款');
  expect(wrapper.text()).toContain('选择套餐不会扣款');
  expect(fetcher).toHaveBeenCalledTimes(2);
  await button(wrapper, '按月学习').trigger('click');
  expect(wrapper.find('[aria-label="套餐开通方式"]').exists()).toBe(false);
});

it('does not sell test, disabled or incomplete products even alongside a valid product', async () => {
  const items = [product, { ...product, product_id: 'draft', title: '未定价', amount_fen: null }, { ...product, product_id: 'test', title: '验收专用', test_only: true }, { ...product, product_id: 'disabled', title: '停用', enabled: false }, { ...product, product_id: 'unavailable', title: '未完整', saleable: false }];
  vi.stubGlobal('fetch', vi.fn(async (url: string) => Response.json(url.endsWith('/storage') ? space : { ...empty, catalog: { items } })));
  const wrapper = start(); await flushPromises(); await button(wrapper, '1 天冲刺').trigger('click');
  expect(wrapper.findAll('.product')).toHaveLength(1);
  expect(wrapper.text()).not.toContain('未定价'); expect(wrapper.text()).not.toContain('验收专用');
});

it('shows sold entitlement dates and next period independently of catalog changes or storage failure', async () => {
  const entitlement = { ...product, product_revision: 1, title: '原已购套餐', period_id: 'p', starts_at: 1792425600, expires_at: 1792512000, receipt_id: 'r', month_anchor_day: null };
  vi.stubGlobal('fetch', vi.fn(async (url: string) => url.endsWith('/storage') ? Response.json({ message: 'offline' }, { status: 503 }) : Response.json({ current: entitlement, next: { ...entitlement, title: '下一期进阶', starts_at: 1792512000, expires_at: 1792771200 }, catalog: { items: [] } })));
  const wrapper = start(); await flushPromises();
  expect(wrapper.text()).toContain('当前：原已购套餐'); expect(wrapper.text()).toContain('已安排下一期：下一期进阶');
  expect(wrapper.text()).toContain('香港时间'); expect(wrapper.text()).toContain('空间用量暂时无法读取');
  expect(wrapper.text()).toContain('生效时发放新一期完整额度');
});

it('clears previous account data and discards its pending response after identity changes', async () => {
  const responses: ((value: Response) => void)[] = [];
  vi.stubGlobal('fetch', vi.fn(() => new Promise<Response>(resolve => responses.push(resolve))));
  const wrapper = start(); await flushPromises();
  installIdentity({ user_id: 'B', csrf_token: 'b' }); await flushPromises();
  responses[0](Response.json(empty)); responses[1](Response.json(space)); await flushPromises();
  expect(wrapper.text()).not.toContain('当前没有生效的云端套餐');
  responses[2](Response.json({ ...empty, catalog: { items: [] } })); responses[3](Response.json(space)); await flushPromises();
  await button(wrapper, '1 天冲刺').trigger('click');
  expect(wrapper.text()).toContain('该时长的套餐尚未开放购买'); expect(wrapper.text()).not.toContain(product.title);
});
