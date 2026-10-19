import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createRequire} from 'node:module';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const web=createRequire(path.join(root,'packages/web/package.json'));
const {build}=createRequire(web.resolve('vite/package.json'))('esbuild');
const methods=path.join(root,'skills/presentation/methods');
const style=fs.readFileSync(path.join(methods,'_shared/linked-v1.css'),'utf8');
const readableModels={
 'discrete-update':'从 x₀=4 出发，在 f(x)=(x−2)² 上做八次更新 xₖ₊₁=xₖ−2η(xₖ−2)。η∈[0,1.2]。当前方框、下一空心圆和橙色画面插值由同一递推序列得到；误差图显示 x−2。0<η<1 收敛，η=0 静止，η=1 等幅往返，η>1 在当前初值下发散。',
 'object-transform':'单位正方形按 x′=x+a·y、y′=y 剪切，a=目标剪切×观察位置，目标剪切∈[−1,1]。四角、水平切片与两个基向量由同一映射得到；面积恒为1，周长为2+2√(1+a²)。',
 'compare-one-factor':'A 固定1 m/s，B为0–3 m/s，同时从距离零点出发。观察位置q对应t=10q秒，距离为速度乘时间，B−A=(speed−1)t米。两条跑道和两条曲线使用同一时刻与固定尺度；t=0时不计算距离比。',
 'part-whole':'总量为100。A=share，B=0.6(100−A)，C=0.4(100−A)。拆开只改变位置；条形长度与百格中的颜色份额保持对应。A=100时B=C=0，此时B/C无定义。',
 'process-reveal':'[2,3,4]依次逐项加增量、保留大于等于阈值的项、对保留项求和。整数位置0/1/2/3依次完成输入展示、map、filter、sum；小数位置只展开连接。输入身份按颜色保持，选中身份不改变结果；空数组的和为0。',
 'evidence-chain':'三组构造耗时记录A=[12,10,11]秒、B=[10,8,9]秒，均值分别11和9秒，差2秒。位置依次展开记录、均值、差值与前提、结论。比较条件不同不能归因于方案；即使条件相同也只描述这三次记录，尚不能推出其他负载或总体优势。',
 'chart-reveal':'构造数据单位为件。positive=[12,18,15,24]，signed=[−6,0,12,24]，zero=[0,6,12,24]。固定零线和−6至30刻度；0≤q<1仅显现柱形，q≥1可读原数，q≥2显示所选类别对A的差与倍数。仅正基数计算增长倍数。12到24是2倍、增加100%；−6到24差30，0到24差24，后两种不计算增长倍数。',
 'scale-zoom':'q∈[0,3]对应10^q倍放大和10^(−q)米视野。目标固定在−0.5至0.5毫米，真实长度1毫米。focus∈[−0.001,0.001]米只改变观察中心。局部标尺占视野宽度五分之一；整体定位图固定宽1米，中心附近的位置轴宽5毫米。'
};
const topics=[
 ['discrete-update',3,8,{},{}],
 ['object-transform',2,1,{shear:.8},{shear:'目标水平剪切系数，−1至1；实际剪切为 shear × 观察位置'}],
 ['compare-one-factor',2,1,{speed:2},{}],
 ['part-whole',2,1,{share:40},{}],
 ['process-reveal',2,3,{increment:1,threshold:4,selected:1},{selected:'被追踪的输入身份，整数0至2；不改变数据或计算结果'}],
 ['evidence-chain',2,3,{controlled:true,selected:0},{selected:'高亮的构造记录，整数0至2；不改变样本或算术结果'}],
 ['chart-reveal',2,3,{dataset:'positive',selected:3},{dataset:'构造数据组 positive=[12,18,15,24]，signed=[−6,0,12,24]，zero=[0,6,12,24]',selected:'与 A 比较的类别，整数0至3；不改变原始数据'}],
 ['scale-zoom',2,3,{focus:0},{focus:'观察中心相对整体原点的位置，米，−0.001至0.001；不改变目标物理长度'}]
];
for(const [id,revision,max,defaults,contracts] of topics){
 const base=path.join(methods,id,`v${revision}`),source=fs.readFileSync(path.join(base,'topic.js'),'utf8');
 const title=/title:'([^']+)'/.exec(source)[1],lead=/lead:'([^']+)'/.exec(source)[1],takeaway=/takeaway:'([^']+)'/.exec(source)[1],limits=/limits:'([^']+)'/.exec(source)[1];
 const old=JSON.parse(fs.readFileSync(path.join(methods,id,`v${revision-1}/example/author.json`),'utf8'));
 const bundle=await build({entryPoints:[path.join(base,'topic.js')],bundle:true,format:'iife',target:'es2020',write:false,minify:false,legalComments:'inline'});
 const html=`<!doctype html>\n<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>${title}</title><style>\n${style}\n</style></head><body><script>\n${bundle.outputFiles[0].text}\n</script></body></html>\n`;
 fs.mkdirSync(path.join(base,'example'),{recursive:true});fs.writeFileSync(path.join(base,'example/index.html'),html);
 const metadata={title,initial_state:{...old.initial_state,...defaults,semantic_state:0,transition_progress:0},state_contract:{...old.state_contract,...contracts,semantic_state:`已完成的解释区间，整数0至${max}；见适配说明中的时间定义`,transition_progress:`当前区间的视觉进度，[0,1)；终点${max}时为0`},readable_content:`${lead} ${readableModels[id]} ${takeaway}`,assumptions:[limits],source_ref_ids:[],libraries:[],asset_refs:[]};
 fs.writeFileSync(path.join(base,'example/author.json'),JSON.stringify(metadata,null,2)+'\n');
 console.log(`Built ${id}@${revision}: ${Buffer.byteLength(html)} bytes`);
}
