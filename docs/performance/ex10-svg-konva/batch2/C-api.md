EX10 condition C: use Konva 10.7.0 + JavaScript for the drawing; HTML controls and text are shared capabilities.
Put the exact marker <!--EX10_KONVA_10.7.0--> before your scene script. The experiment host replaces only this marker with the fixed inline library before write; do not copy the library or use script src. Its 192898 bytes count toward the existing 1 MiB candidate limit.
Create a Konva.Stage({container:'drawing',width,height}), add a Konva.Layer, then add shapes. Circle: x,y,radius; Line: points:[x1,y1,x2,y2]; Arrow: points,pointerLength,pointerWidth; Group: x,y,scale; common: fill,stroke,strokeWidth,id,name. Set node.position({x,y}), line.points([...]), node.setAttrs({...}); layer.draw() redraws immediately; batchDraw schedules redraw. stage.findOne(selector) finds a node; destroy removes it. Set draggable:true; node.on('dragmove',handler)/on('dragend',handler) handle drag, dragBoundFunc constrains absolute position. getAbsoluteTransform().point({x,y}) maps logical coordinates. Resize stage.width/height and stage.scale consistently. HTML range input's input event changes parameters live; change marks completion.
Unrelated moving-point example (HTML has input#parameter and div#drawing, marker already loaded):
const stage=new Konva.Stage({container:'drawing',width:280,height:100});
const layer=new Konva.Layer();stage.add(layer);
const point=new Konva.Circle({x:30,y:50,radius:10,fill:'#2467ab'});layer.add(point);
let a=100,t=0,playing=false,last=null;
function renderAt(time){t=time;point.x(30+a*t);layer.draw();}
document.querySelector('#parameter').oninput=e=>{playing=false;a=Number(e.target.value);renderAt(0);};
function tick(now){if(!playing)return;if(last!==null)renderAt(Math.min(1,t+(now-last)/1000));last=now;if(t<1)requestAnimationFrame(tick);else playing=false;}
function play(){playing=true;last=null;requestAnimationFrame(tick);}
function pause(){playing=false;}
function seek(time){pause();renderAt(time);}
This generic example illustrates movement, parameter input and pause/positioning; construct the requested explanation yourself.
