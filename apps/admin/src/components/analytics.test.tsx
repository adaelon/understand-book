import { expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { AllowanceValue, Amount, Distribution, overviewMoney, overviewPercentage } from './analytics';

it('keeps unknown and sub-cent costs distinct from zero while preserving exact amounts', () => {
  expect(overviewMoney(null)).toBe('未知');
  expect(overviewMoney(0)).toBe('0.00');
  expect(overviewMoney(5000)).toBe('< 0.01');
  expect(overviewMoney(3903378)).toBe('3.90');
  expect(overviewMoney(-230, 2)).toBe('-2.30');
  expect(renderToStaticMarkup(<Amount value={3903378}/>)).toContain('精确金额：3.903378 元');
});

it('shows money for an unconfigured legacy allowance and keeps long percentages precise in details', () => {
  expect(renderToStaticMarkup(<AllowanceValue ratio={null} amount={10000000}/>)).toContain('¥ 10.00');
  const ratio = { value: '19.038106', nonzero: true, below_tenth: false };
  const markup = renderToStaticMarkup(<AllowanceValue ratio={ratio} amount={19038106}/>);
  expect(markup).toContain('19.04%');
  expect(markup).toContain('精确比例：19.038106%');
  expect(overviewPercentage({value:'130',nonzero:true,below_tenth:false})).toBe('130%');
  expect(overviewPercentage({value:'0.000001',nonzero:true,below_tenth:true})).toBe('低于 0.1%');
});

it('shows a grant above 100% without claiming its available quota is full or losing the fixed basis', () => {
  const chart=renderToStaticMarkup(<Distribution label="本期额度分配" total={150} segments={[
    {label:'已扣减',value:20,display:'20%',tone:'blue'},
    {label:'可用',value:130,display:'130%',tone:'green'},
  ]}/>);
  expect(chart).toContain('可用 130%');
  expect(chart).toContain('width:86.66666666666667%');
  const empty=renderToStaticMarkup(<Distribution label="无调用" segments={[{label:'已用',value:0,tone:'blue'}]}/>);
  expect(empty).not.toMatch(/NaN|Infinity|width:100%/);
});
