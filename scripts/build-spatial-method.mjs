import fs from 'node:fs';
import path from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const base=path.join(root,'skills/presentation/methods/spatial-linked-views/v1');
const web=createRequire(path.join(root,'packages/web/package.json'));
const {build}=createRequire(web.resolve('vite/package.json'))('esbuild');
const range=(key,name,min,max,step)=>`<label for="${key}">${name}<output id="${key}-value"></output><input id="${key}" data-param="${key}" type="range" min="${min}" max="${max}" step="${step}"></label>`;
const topics={
  gear:{file:'example/index.html',title:'齿轮的约定',number:'01',sceneTitle:'从结构看见传动',
    intro:'两只齿轮并没有约定谁应该快、谁应该慢。齿的接触，替它们约定了一切。转动视角，再改变齿数，看同一件事怎样出现在三维结构、平面图和数字里。',
    controls:'<label for="teeth">输出齿数<select id="teeth" data-param="teeth"><option value="20">20 齿</option><option value="30">30 齿</option><option value="40" selected>40 齿</option><option value="60">60 齿</option></select></label>'+range('rpm','输入转速',0,24,1)+range('torque','输入扭矩',0,2,.1),
    legend:'<span class="gold">● 金色：输入轮</span><span class="teal">● 青色：输出轮</span>',
    diagramTitle:'同一点，同一速度',detailTitle:'更长的力臂，更大的扭矩',
    diagramCaption:'白点追踪同一颗齿的位置。拖动金色齿轮，或用方向键微调，三维结构随之转动。虚线表示节圆。',
    detailCaption:'两轮接触处传递相同大小的切向力。输出轮的半径越大，这个力产生的扭矩就越大；两根箭头使用同一尺度。',
    question:'为什么变慢，却变得更有力？',answer:'接触的齿不能互相穿过。两个节圆相切处的线速度相同，而更大的圆每转一圈，要走更长的距离。所以大轮转得更慢。同一个力作用在更长的力臂上，又会产生更大的扭矩。',
    formula:'ω₂ / ω₁ = − z₁ / z₂　　τ₂ / τ₁ = z₂ / z₁',
    takeaway:'改变输出齿数，只改变速度与扭矩的交换比例。理想条件下，输入与输出功率相等。把输入转速或扭矩设为零，观察哪些关系仍然成立。',
    assumptions:'模型：20° 标准渐开线直齿轮，模数 2 mm，输入 20 齿；忽略摩擦、弹性与惯性，所示扭矩为理想传递值。根部过渡作几何简化。拆解不代表可运行装配。',
    initial:{teeth:40,rpm:12,torque:2},contract:{teeth:'输出齿数；20、30、40、60；输入固定20齿',rpm:'输入转速；rpm；0至24',torque:'输入扭矩；N·m；0至2'},
    readable:'标准直齿轮的节圆相切处线速度相同。输出转速等于输入转速乘以负的输入齿数/输出齿数。理想输出扭矩等于输入扭矩乘以输出齿数/输入齿数，功率守恒。三维模型、可拖动的节圆图和扭矩图由同一模型状态驱动。'},
  spring:{file:'adaptations/spring.html',title:'能量来回的路',number:'02',sceneTitle:'一根弹簧，一块质量',
    intro:'把质量块拉开，再松手。它为什么越过中点，又为什么会折返？转动模型，沿曲线移动，再观察能量怎样从一种形式变成另一种。',
    controls:range('mass','质量',.5,2,.1)+range('stiffness','劲度系数',4,20,1)+range('amplitude','振幅',0,.8,.05),
    legend:'<span class="gold">● 金色：弹簧与势能</span><span class="teal">● 青色：质量块与动能</span>',
    diagramTitle:'每一圈，回到相同位置',detailTitle:'总量不变，形式在变',
    diagramCaption:'横轴是一个完整周期，纵轴是相对平衡位置的位移。拖动曲线上的观察线，三维质量块与能量柱同时改变。',
    detailCaption:'金色是弹簧势能，青色是质量块动能，灰色是它们的和。使用固定能量尺度，改变振幅时也能直接比较。',
    question:'到中点，为什么还不停下？',answer:'两端是位移的极值，速度为零，能量全在弹簧里。质量块向中点运动时，势能不断转成动能。经过中点的一刻，回复力虽然为零，速度却最大，于是质量块继续前进，直到动能重新变成势能。',
    formula:'x = A cos(ωt)　　ω = √(k/m)　　E = ½kA²',
    takeaway:'增大质量会让往复更慢；增大劲度系数会让往复更快。改变振幅会改变总能量，在这个理想模型中却不改变周期。把振幅设为零，整套图解会停在平衡位置。',
    assumptions:'模型：水平、无阻尼、无摩擦的线性弹簧振子，从最大正位移静止释放。导轨限制为一维运动；弹簧的线圈外观和质量块尺寸作示意，不表示实际材料或质量密度。拆解时暂停运动。',
    initial:{mass:1,stiffness:12,amplitude:.6},contract:{mass:'质量；kg；0.5至2',stiffness:'劲度系数；N/m；4至20',amplitude:'初始最大正位移；m；0至0.8'},
    readable:'无阻尼线性弹簧振子满足x=Acos(√(k/m)t)，周期2π√(m/k)。势能为kx²/2，动能为mv²/2，总能量为kA²/2。质量块、位移曲线和两种能量共享同一时刻；相机和拆解只改变观察方式。'}
};
const shell=fs.readFileSync(path.join(base,'shell.html'),'utf8'),style=fs.readFileSync(path.join(base,'style.css'),'utf8');
for(const [name,topic] of Object.entries(topics)){
  const bundle=await build({entryPoints:[path.join(base,`${name}.js`)],bundle:true,format:'iife',target:'es2020',write:false,minify:false});
  const props={...topic,style,script:bundle.outputFiles[0].text,library:'libraries/three-0.180.0.min.js'};
  const html=shell.replace(/\{\{(\w+)\}\}/g,(_,key)=>props[key]);
  const dest=path.join(base,topic.file);fs.mkdirSync(path.dirname(dest),{recursive:true});fs.writeFileSync(dest,html);
  const metadata={title:topic.title,initial_state:{...topic.initial,yaw:-.28,tilt:.62,explode:0,semantic_state:0,transition_progress:0},
    state_contract:{...topic.contract,yaw:'观察方位角；弧度；不改变物理模型',tilt:'观察倾角；弧度；0至1.22',explode:'沿轴拆解比例；0至1；不改变物理参数',semantic_state:'已完成的周期数；整数0至4',transition_progress:'当前周期分数；0至1，不含1；终点归入下一周期'},
    readable_content:topic.readable,assumptions:[topic.assumptions],source_ref_ids:[],libraries:['three'],asset_refs:[]};
  fs.writeFileSync(dest.replace(/(?:index|spring)\.html$/,name==='gear'?'author.json':'spring-author.json'),JSON.stringify(metadata,null,2)+'\n');
}
const vendor=path.join(root,'skills/presentation/methods/vendor');fs.mkdirSync(vendor,{recursive:true});
fs.copyFileSync(path.join(root,'assets/presentation/three-0.180.0/three.min.js'),path.join(vendor,'three-0.180.0.min.js'));
fs.copyFileSync(path.join(root,'assets/presentation/three-0.180.0/LICENSE'),path.join(vendor,'THREE-LICENSE.txt'));
console.log('Built spatial-linked-views@1: gear + spring, shared runtime, Author metadata and offline preview library');
