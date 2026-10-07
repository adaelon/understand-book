import {test,expect} from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
const host='http://127.0.0.1:'+(process.env.PRESENTATION_TEST_PORT || '4175');

// Only observations depend on the generated representation. Oracles use the
// supplied closed form / circle geometry and the labelled coordinate axes.
test('EX11 batch8 independent mathematics and rendered geometry',async({browser})=>{
  test.setTimeout(120000);
  const root=path.resolve(process.env.EX11_RUN_DIR!);
  const kind=path.basename(root), records:any[]=[];
  for(const viewport of [{width:960,height:720},{width:320,height:420},{width:640,height:240}]){
    const context=await browser.newContext({viewport});const page=await context.newPage();
    await page.request.post(host+'/reset-scene');
    await page.route('**/api/**',async route=>{
      const response=await route.fetch({url:route.request().url().replace(/^.*\/api/,host)});
      await route.fulfill({response});
    });
    await page.goto('/agent-presentation-visual.html');
    const frame=page.frameLocator('.agent-presentation iframe');
    await expect(frame.locator(kind==='disk-1'?'#theta':'#eta')).toBeVisible();
    await page.getByRole('button',{name:'展开',exact:true}).click();
    if(kind==='disk-1'){
      for(const radius of [1,1.5,2,3]) for(const q of [0,.35,1,2.35,4,6.8,8]){
        const actual=await frame.locator('body').evaluate(async(_,v)=>{
          const control=document.querySelector('#r2') as HTMLInputElement;
          control.value=String(v.radius);control.dispatchEvent(new Event('input',{bubbles:true}));
          await (window as any).presentationScene.seek({semantic_state:Math.floor(v.q),transition_progress:v.q%1});
          const disks=['#svgA','#svgB'].map(id=>{
            const svg=document.querySelector(id)!;
            const rim=svg.querySelector('.rim')!,ray=svg.querySelector('.radiusRay')!,arc=svg.querySelector('.arc') as SVGPathElement;
            return {cx:+rim.getAttribute('cx')!,cy:+rim.getAttribute('cy')!,r:+rim.getAttribute('r')!,x:+ray.getAttribute('x2')!,y:+ray.getAttribute('y2')!,length:arc.style.display==='none'?0:arc.getTotalLength()};
          });
          const strip=document.querySelector('#svgS')!;
          return {disks,bar1:+strip.querySelector('.bar1')!.getAttribute('width')!,bar2:+strip.querySelector('.bar2')!.getAttribute('width')!,span:+strip.querySelector('.axis')!.getAttribute('x2')!-12,
            text:document.querySelector('#readout')!.textContent,scene:(window as any).presentationScene.snapshot()};
        },{radius,q});
        const theta=q*Math.PI/4,errors:string[]=[];
        for(const [i,d] of actual.disks.entries()){
          if(Math.abs(d.x-(d.cx+d.r*Math.cos(theta)))>.025||Math.abs(d.y-(d.cy-d.r*Math.sin(theta)))>.025)errors.push(`disk${i} endpoint`);
          // Chromium's flattened SVG arc length is approximate (even for an
          // exact two-semicircle circle); allow half a CSS pixel over the arc.
          if(Math.abs(d.length-d.r*theta)>.5)errors.push(`disk${i} arc length`);
        }
        if(Math.abs(actual.disks[1].r/actual.disks[0].r-radius)>.002)errors.push('radius ratio');
        if(Math.abs(actual.bar1/actual.span*6*Math.PI-theta)>.009||Math.abs(actual.bar2/actual.span*6*Math.PI-radius*theta)>.009)errors.push('unrolled length');
        records.push({viewport,radius,q,theta,actual,errors});
      }
    }else{
      for(const eta of [0,.2,.5,.8,1,1.1,1.2]){
        const horizon=await frame.locator('#eta').evaluate((node,eta)=>{
          (node as HTMLInputElement).value=String(eta);node.dispatchEvent(new Event('input',{bubbles:true}));
          return Number((document.querySelector('#pos') as HTMLInputElement).max);
        },eta);
        for(const k of [0,1,2,3,5]) for(const progress of [0,.35,.8]){
          if(k+progress>horizon)continue;
          const q=k+progress,wk=2-2*Math.pow(1-2*eta,k),wn=2-2*Math.pow(1-2*eta,k+1),w=wk+(wn-wk)*progress;
          const actual=await frame.locator('body').evaluate(async(_,v)=>{
            // The second draft exposes a wrong two-argument seek API. Keep its
            // API failure separate; drive the real user slider to test its math.
            if(v.kind==='learning-2'){
              const pos=document.querySelector('#pos') as HTMLInputElement;
              pos.value=String(v.k+v.progress);pos.dispatchEvent(new Event('input',{bubbles:true}));
            }else await (window as any).presentationScene.seek({semantic_state:v.k,transition_progress:v.progress});
            if(v.kind==='learning-1'){
              const lane=document.querySelector('#lane')!,traj=document.querySelector('#traj')!;
              const dot=lane.querySelector('circle[r="4.8"]')!,td=traj.querySelector('circle[r="5"]')!;
              return {text:document.querySelector('#readNums')!.textContent,laneW:+lane.getAttribute('width')!,x:+dot.getAttribute('cx')!,y:+dot.getAttribute('cy')!,trajW:+traj.getAttribute('width')!,tx:+td.getAttribute('cx')!,ty:+td.getAttribute('cy')!};
            }
            const stage=(window as any).Konva.stages[0];
            const points=stage.find('Circle').filter((n:any)=>n.radius()===6.5).map((n:any)=>({x:n.x(),y:n.y(),visible:n.visible()}));
            return {text:document.querySelector('#readout')!.textContent,width:stage.width(),points,arrows:stage.find('Arrow').filter((n:any)=>n.visible()).length};
          },{kind,k,progress});
          const errors:string[]=[];const close=(a:number,b:number,name:string,tol=.015)=>{if(Math.abs(a-b)>tol)errors.push(`${name}: ${a} != ${b}`);};
          const shown=Number(actual.text!.match(/w\s*=\s*([-+\d.eE]+)/)![1]);
          close(shown,w,'readout',Math.max(.0006,Math.abs(w)*.001));
          if(kind==='learning-1'){
            const D=Math.min(8,Math.max(2,Math.max(...Array.from({length:horizon+1},(_,t)=>2*Math.abs(Math.pow(1-2*eta,t))))*1.25));
            const clamp=(x:number,a:number,b:number)=>Math.max(a,Math.min(b,x));
            close(actual.x!,clamp(12+(w-2+D)/(2*D)*(actual.laneW!-24),12,actual.laneW!-12),'lane x');
            close(actual.y!,110,'lane y');
            const left=actual.trajW!<430?36:46;
            close(actual.tx!,left+q/Math.max(horizon,8)*(actual.trajW!-left-14),'trajectory x');
            const e=w-2,y=Math.abs(e)<.001?99:99-Math.sign(e)*Math.min(6,Math.log10(Math.abs(e))+3)*83/6;
            close(actual.ty!,y,'trajectory y');
          }else{
            const left=actual.width!<430?38:46,right=actual.width!<430?12:16,span=actual.width!-left-right;
            const moving=actual.points!.find((p:any)=>p.y===200)!;
            close(moving.x,left+(Math.max(-10,Math.min(14,w))+10)/24*span,'moving x');
            const trajectory=actual.points!.find((p:any)=>p.y!==200)!;
            close(trajectory.x,left+q/12*span,'trajectory x');
            close(trajectory.y,418-(Math.max(-10,Math.min(14,w))+10)/24*134,'trajectory y');
            if(Math.abs(wn-wk)<1e-12&&actual.arrows!==0)errors.push('zero displacement arrow');
          }
          records.push({viewport,eta,k,progress,horizon,expected:w,actual,errors});
        }
      }
    }
    fs.writeFileSync(path.join(root,'independent-math.json'),JSON.stringify(records,null,2));
    await frame.locator(kind==='disk-1'?'#svgS':kind==='learning-1'?'#lane':'#stage').scrollIntoViewIfNeeded();
    await page.screenshot({path:path.join(root,`math-${viewport.width}.png`)});
    await page.unrouteAll({behavior:'ignoreErrors'});await context.close();
  }
  expect(records.filter(r=>r.errors.length).map(({viewport,eta,k,progress,radius,q,errors})=>({viewport,eta,k,progress,radius,q,errors}))).toEqual([]);
});
