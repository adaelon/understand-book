EX10 condition B: use native SVG + JavaScript for the drawing; HTML controls and text are shared capabilities.
Create SVG nodes with document.createElementNS('http://www.w3.org/2000/svg',tag), append to an svg with viewBox, update attributes with setAttribute. circle: cx,cy,r; line: x1,y1,x2,y2; polyline: points; path: d; g: transform; arrowheads: marker-end referencing a marker. querySelector finds shapes; remove removes them. SVG scales with its viewport; getScreenCTM transforms logical geometry to screen coordinates. Use pointerdown/move/up and setPointerCapture for custom dragging, convert pointer coordinates back with inverse CTM. Use HTML range input's input event for live parameter changes, change for interaction completion.
Unrelated moving-point example (HTML has input#parameter and svg#drawing):
const svg=document.querySelector('#drawing');
svg.setAttribute('viewBox','0 0 280 100');
const point=document.createElementNS('http://www.w3.org/2000/svg','circle');
for(const [key,value] of Object.entries({cx:30,cy:50,r:10,fill:'#2467ab'}))point.setAttribute(key,value);
svg.append(point);
let a=100,t=0,playing=false,last=null;
function renderAt(time){t=time;point.setAttribute('cx',String(30+a*t));}
document.querySelector('#parameter').oninput=e=>{playing=false;a=Number(e.target.value);renderAt(0);};
function tick(now){if(!playing)return;if(last!==null)renderAt(Math.min(1,t+(now-last)/1000));last=now;if(t<1)requestAnimationFrame(tick);else playing=false;}
function play(){playing=true;last=null;requestAnimationFrame(tick);}
function pause(){playing=false;}
function seek(time){pause();renderAt(time);}
This generic example illustrates movement, parameter input and pause/positioning; construct the requested explanation yourself.
