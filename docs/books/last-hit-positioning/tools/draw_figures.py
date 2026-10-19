"""用 Pillow 按真实算例坐标绘制本书的几何示意图。"""
from pathlib import Path
import math
import sys
from PIL import Image, ImageDraw, ImageFont

BOOK = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(BOOK / "examples"))
from book_cases import BASE, geometry, window, window_trace
from last_hit_positioning_v3 import solve_geometry

BG = "#fbfaf6"
INK = "#25364b"
MUTED = "#687782"
RED = "#c75b54"
BLUE = "#28699f"
GOLD = "#bb8825"
GREEN = "#287e69"
PURPLE = "#785d9c"
FONT = "C:/Windows/Fonts/msyh.ttc"
BOLD = "C:/Windows/Fonts/msyhbd.ttc"


def font(size, bold=False):
    return ImageFont.truetype(BOLD if bold else FONT, size)


def label(draw, xy, value, color=INK, size=25, bold=False, anchor=None):
    draw.text(xy, value, fill=color, font=font(size, bold), anchor=anchor)


def wrap(draw, x, y, text, width, size=25, color=INK, gap=11):
    line = ""
    for ch in text:
        if ch == "\n" or draw.textlength(line + ch, font=font(size)) > width:
            label(draw, (x, y), line, color, size)
            y += size + gap
            line = "" if ch == "\n" else ch
        else:
            line += ch
    if line:
        label(draw, (x, y), line, color, size)
        y += size + gap
    return y


class Plot:
    def __init__(self, width=1160, height=740, limits=(-7, 12, -7, 12)):
        self.image = Image.new("RGB", (width, height), BG)
        self.draw = ImageDraw.Draw(self.image)
        self.width, self.height = width, height
        self.x0, self.x1, self.y0, self.y1 = limits
        self.scale = min((width - 120) / (self.x1 - self.x0), (height - 80) / (self.y1 - self.y0))
        self.left = (width - (self.x1 - self.x0) * self.scale) / 2
        self.top = (height - (self.y1 - self.y0) * self.scale) / 2

    def xy(self, point):
        x, y = point
        return (self.left + (x - self.x0) * self.scale,
                self.top + (self.y1 - y) * self.scale)

    def circle(self, center, radius, outline, fill=None, width=3):
        x, y = self.xy(center)
        r = radius * self.scale
        self.draw.ellipse((x-r, y-r, x+r, y+r), fill=fill, outline=outline, width=width)

    def line(self, points, color, width=4, dashed=False):
        pts = [self.xy(p) for p in points]
        if dashed:
            for start, end in zip(pts, pts[1:]):
                dx, dy = end[0]-start[0], end[1]-start[1]
                length = math.hypot(dx, dy)
                for i in range(0, int(length), 18):
                    t0, t1 = i/length, min((i+10)/length, 1)
                    self.draw.line((start[0]+dx*t0, start[1]+dy*t0,
                                    start[0]+dx*t1, start[1]+dy*t1), fill=color, width=width)
        else:
            self.draw.line(pts, fill=color, width=width, joint="curve")

    def grid(self):
        for x in range(math.ceil(self.x0/2)*2, math.floor(self.x1)+1, 2):
            self.line([(x, self.y0), (x, self.y1)], "#e7e9e6", 1)
            label(self.draw, self.xy((x, self.y0)), str(x), MUTED, 18, anchor="mt")
        for y in range(math.ceil(self.y0/2)*2, math.floor(self.y1)+1, 2):
            self.line([(self.x0, y), (self.x1, y)], "#e7e9e6", 1)
            xx, yy = self.xy((self.x0, y))
            label(self.draw, (xx-12, yy), str(y), MUTED, 18, anchor="rm")
        self.line([(0, self.y0), (0, self.y1)], "#b5bec2", 2)
        self.line([(self.x0, 0), (self.x1, 0)], "#b5bec2", 2)

    def dot(self, p, name, color, shape="circle", offset=(12, -28), size=9):
        x, y = self.xy(p)
        if shape == "square":
            self.draw.rectangle((x-size, y-size, x+size, y+size), fill=color, outline=BG, width=2)
        elif shape == "diamond":
            self.draw.polygon(((x,y-size-3),(x+size+3,y),(x,y+size+3),(x-size-3,y)), fill=color)
        else:
            self.draw.ellipse((x-size, y-size, x+size, y+size), fill=color, outline=BG, width=2)
        if name:
            label(self.draw, (x+offset[0], y+offset[1]), name, color, 25, True)


def frame(title, subtitle, plot, note, filename):
    image = Image.new("RGB", (1200, plot.height + 205), BG)
    draw = ImageDraw.Draw(image)
    label(draw, (44, 25), title, INK, 34, True)
    label(draw, (46, 78), subtitle, MUTED, 23)
    image.paste(plot.image, ((1200-plot.width)//2, 120))
    draw.line((44, plot.height+133, 1156, plot.height+133), fill="#d7dddc", width=2)
    wrap(draw, 46, plot.height+149, note, 1100, 23)
    image.save(BOOK / "assets" / filename)


def static_plot(g, width=1160, height=740, limits=(-7, 12, -7, 12), cast=False):
    p = Plot(width, height, limits)
    p.circle((g.E.x,g.E.y), g.enemy_aa, RED, "#f6e8e2")
    p.circle((g.T.x,g.T.y), g.self_aa, GOLD, "#faf0d5")
    p.grid()
    p.circle((g.E.x,g.E.y), g.safety_radius, "#dca799", width=2)
    p.circle((g.T.x,g.T.y), g.self_aa, GOLD)
    if cast:
        p.circle((g.E.x,g.E.y), g.self_cast, PURPLE, width=2)
    p.dot((g.E.x,g.E.y), "对手", RED, offset=(-66,18))
    p.dot((g.F.x,g.F.y), "我方兵", "#76a8b7", "square", offset=(-28,20))
    p.dot((g.T.x,g.T.y), "目标兵", GOLD, "diamond", offset=(-82,14))
    return p


def candidate_line(p, g, interval, color=GREEN):
    p.line([(g.Q.x,-0.6), (g.Q.x,p.y1-0.3)], MUTED, 2, True)
    if interval is not None and not interval.empty:
        p.line([(g.point(interval.lo).x,g.point(interval.lo).y),
                (g.point(interval.hi).x,g.point(interval.hi).y)], color, 9)


def scene():
    g = geometry("G1")
    p = static_plot(g)
    p.dot((6.5,4), "你", BLUE, offset=(20,-25), size=11)
    p.line([(0,0),(6.5,4)], RED, 2, True)
    p.line([(6,4),(6.5,4)], BLUE, 4)
    label(p.draw, p.xy((-5.7,4.5)), "对手普攻 6 格", RED, 24)
    label(p.draw, p.xy((6.8,8.2)), "补兵距离 3 格", GOLD, 24)
    frame("先认清目标与对手", "同一个位置，需要同时看两段距离", p,
          "红圈：对手能普攻到你的位置。金圈：你能普攻目标兵的站位。练习单位：格。", "01-scene.png")


def positions():
    g = geometry("G1")
    p = static_plot(g)
    for name, pos, offset in (("A",(6.5,1),(16,-18)),("B",(6.5,4),(18,-23)),
                              ("C",(6.5,7),(16,-20)),("D",(3,4),(-40,-30))):
        p.dot(pos, name, BLUE, offset=offset)
    frame("同一只兵，四个候选位置", "A、C 差一点够到兵；B 两个距离都合适；D 进入对手普攻范围", p,
          "B 到兵 0.50 格、到对手约 7.63 格。D 到兵 3.00 格、到对手 5.00 格。", "02-positions.png")


def candidate():
    g = geometry("G1")
    p = static_plot(g)
    candidate_line(p, g, g.safeAaI)
    p.dot((g.Q.x,0), "最近处 6.5 格", GREEN, offset=(18,15))
    p.dot((6.5,4), "B", BLUE, offset=(24,-20))
    for s, text in ((g.safeAaI.lo,"下端约 1.04"),(g.safeAaI.hi,"上端约 6.96")):
        p.dot((6.5,s), text, GREEN, offset=(23,-16), size=6)
    frame("沿线寻找一段可补刀的位置", "候选线在 6 格普攻范围外，再留半格余量", p,
          "目标到线的距离是 0.5 格。绿色区间来自 √(3²−0.5²)，并非只有 B 一个位置。", "03-candidate.png")


def options():
    image = Image.new("RGB", (1200,1900), BG)
    draw = ImageDraw.Draw(image)
    label(draw, (44,26), "目标移动，哪些要求还能同时满足？", INK, 34, True)
    rows = [
        ("G1", "01  补刀与施法参考同时满足", "目标 (6,4)\n绿色：普攻候选\n紫色：再加 8 格施法参考\n交集约 1.04—4.66"),
        ("G2", "02  先保留普攻补刀", "目标 (6,9)\n绿色：仍能普攻补兵\n候选都超出 8 格参考范围\n普攻区间约 6.04—11.96"),
        ("G3", "03  在线上比较技能方案", "目标 (2,4)\n蓝色：5 格技能的候选\nP：线外仍有普攻位置\nP 到对手 6.89，到兵 2.42"),
    ]
    for i,(case_id,title,note) in enumerate(rows):
        g=geometry(case_id)
        p=static_plot(g,780,510,(-1,11,-1,13),cast=case_id!="G3")
        candidate_line(p,g,g.safeAaI)
        if case_id=="G1":
            p.line([(6.5,g.legalI.lo),(6.5,g.legalI.hi)],PURPLE,5)
        if case_id=="G3":
            candidate_line(p,g,g.skillSafeI,BLUE)
            p.dot((3,6.2),"P",GREEN,offset=(-37,-26))
        y=90+i*590
        label(draw,(44,y),title,INK,28,True)
        image.paste(p.image,(10,y+55))
        wrap(draw,825,y+130,note,330,25,gap=16)
        draw.line((44,y+570,1156,y+570),fill="#d7dddc",width=2)
    label(draw,(46,1860),"三个场景：敌方普攻 6 格，我方普攻 3 格，额外余量 0.5 格。",MUTED,23)
    image.save(BOOK/"assets/04-options.png")


def skills():
    image=Image.new("RGB",(1200,1100),BG)
    draw=ImageDraw.Draw(image)
    label(draw,(44,26),"圆形的半径与直线的宽度，各有自己的方向",INK,32,True)
    for index in (0,1):
        p=Plot(1100,350,(-1,7,-2,2))
        p.grid()
        if index==0:
            p.circle((4,0),1.5,GOLD,"#faf0d5")
            p.circle((6,0),.5,BLUE,"#e7f0f7")
            p.line([(0,0),(4,0)],MUTED,3,True)
            p.dot((0,0),"施法者",RED,offset=(-30,20))
            p.dot((4,0),"圆心",GOLD,offset=(-25,20),size=5)
            p.dot((6,0),"目标中心",BLUE,offset=(-35,60),size=5)
            label(p.draw,p.xy((1.1,0.6)),"圆心最远 4",MUTED,23)
            label(p.draw,p.xy((3.2,1.9)),"作用半径 1.5",GOLD,23)
            note="自由瞄准时，最远中心可达：4 + 1.5 + 0.5 = 6 格。"
        else:
            a=p.xy((0,1)); b=p.xy((4,-1))
            p.draw.rectangle((a[0],a[1],b[0],b[1]),fill="#faf0d5",outline=GOLD,width=3)
            p.circle((4.5,0),.5,BLUE,"#e7f0f7")
            p.line([(0,0),(4,0)],MUTED,2,True)
            p.dot((0,0),"施法者",RED,offset=(-30,25))
            label(p.draw,p.xy((1.1,1.7)),"纵向长度 4",GOLD,24)
            label(p.draw,p.xy((.4,-1.3)),"完整宽度 2",GOLD,24)
            label(p.draw,p.xy((4.6,.8)),"目标半径 0.5",BLUE,24)
            note="沿中心线瞄准的纵向可达：4 + 0.5 = 4.5 格；宽度不加到前方。"
        y=110+index*480
        label(draw,(44,y),"圆形技能" if index==0 else "直线技能",INK,28,True)
        image.paste(p.image,(50,y+45))
        label(draw,(60,y+405),note,MUTED,23)
    image.save(BOOK/"assets/05-skills.png")


def timeline():
    image=Image.new("RGB",(1200,560),BG)
    draw=ImageDraw.Draw(image)
    label(draw,(44,28),"1.2 秒里，双方都在行动",INK,34,True)
    label(draw,(46,82),"我方：上前 1.5 格，移速 5 格/秒；反应与前摇各 0.2 秒",MUTED,24)
    left,right,y=90,1110,225
    scale=(right-left)/1.2
    segments=[(0,.2,"反应","#e5e7e8"),(.2,.5,"上前","#a9cbdf"),(.5,.7,"前摇","#ead7aa"),
              (.7,1.0,"返回","#9fcbbb"),(1.0,1.2,"继续退","#bbd7c8")]
    for a,b,text,color in segments:
        x0,x1=left+a*scale,left+b*scale
        draw.rounded_rectangle((x0+2,y,x1-2,y+72),radius=8,fill=color)
        label(draw,((x0+x1)/2,y+36),text,INK,26,True,anchor="mm")
    for t in (0,.2,.5,.7,1.0,1.2):
        x=left+t*scale
        draw.line((x,y-20,x,y-2),fill=MUTED,width=2)
        label(draw,(x,y-40),f"{t:g} 秒",MUTED,23,anchor="mm")
    for t,text,color in ((.7,"普攻出手",GOLD),(1.0,"回到等待点",GREEN)):
        label(draw,(left+t*scale,y+108),text,color,23,True,anchor="mm")
    draw.line((left,430,right,430),fill=RED,width=6)
    label(draw,(left,378),"对手从出手后立刻追击",RED,25,True)
    label(draw,(right,470),"1.2 秒：下一次普攻可开始",RED,25,anchor="rm")
    image.save(BOOK/"assets/06-timeline.png")


def chase_plot(case_id,width=1060,height=670):
    g,sim=window(case_id)
    trace=window_trace(case_id)
    p=Plot(width,height,(-3,13,-4,14))
    p.grid()
    p.line([(row[3],row[4]) for row in trace],RED,5)
    p.line([(row[1],row[2]) for row in trace],BLUE,5)
    final=trace[-1]
    p.circle((final[3],final[4]),6,RED,width=2)
    p.line([(final[1],final[2]),(final[3],final[4])],MUTED,2,True)
    p.dot((0,0),"对手起点",RED,offset=(-90,20))
    p.dot((final[3],final[4]),"对手终点",RED,offset=(-125,-28))
    p.dot((sim.staging.x,sim.staging.y),"等待点",BLUE,offset=(18,20),size=7)
    p.dot((final[1],final[2]),"你的终点",BLUE,offset=(15,-30),size=9)
    p.dot((g.T.x,g.T.y),"目标兵",GOLD,"diamond",offset=(-70,-36))
    return p,sim


def chase():
    image=Image.new("RGB",(1200,1710),BG)
    draw=ImageDraw.Draw(image)
    label(draw,(44,28),"同样按时返回，距离却不一样",INK,34,True)
    for i,case in enumerate(("W1","W2")):
        p,sim=chase_plot(case,1100,650)
        y=100+i*785
        title="场景一  横向更远" if i==0 else "场景二  横向更近"
        label(draw,(44,y),title,INK,29,True)
        image.paste(p.image,(50,y+50))
        label(draw,(65,y+710),f"0.7 秒出手 · 1.0 秒返回 · 1.2 秒距离 {sim.dist_ready:.2f} 格",GREEN if sim.window_valid else RED,27,True)
    image.save(BOOK/"assets/07-chase.png")


def short_range():
    image=Image.new("RGB",(1200,1720),BG)
    draw=ImageDraw.Draw(image)
    label(draw,(44,28),"对手普攻 6 格，你的普攻 2 格",INK,34,True)
    specs=[((6,10),"第一只兵  直接补", "等待点到兵 0.5 格，已经够得到。"),
           ((3,10),"第二只兵  上前再退", "差 1.5 格；按计划返回后仍有距离。"),
           ((3,5),"第三只兵  重新选择", "也差 1.5 格；同样返回却被追进范围。")]
    for i,(target,title,note) in enumerate(specs):
        g=solve_geometry(**(BASE | dict(target_minion=target,self_aa=2,skill_range=5)))
        p=static_plot(g,760,440,(-1,11,-1,13))
        candidate_line(p,g,g.safeAaI)
        st=g.staging_point()
        p.dot((st.x,st.y),"等待点",BLUE,offset=(16,-28))
        if i:
            p.line([(st.x,st.y),(5,target[1])],BLUE,6)
            p.dot((5,target[1]),"出手点",BLUE,offset=(-40,22),size=6)
        y=110+i*520
        label(draw,(44,y),title,INK,28,True)
        image.paste(p.image,(25,y+50))
        wrap(draw,820,y+190,note,310,26,gap=16)
    label(draw,(46,1670),"示意图只改变目标兵位置。完整时间和距离见正文与附录 B。",MUTED,23)
    image.save(BOOK/"assets/08-short-range.png")


def main():
    (BOOK/"assets").mkdir(exist_ok=True)
    for draw in (scene,positions,candidate,options,skills,timeline,chase,short_range):
        draw()
    print("Generated 8 original, coordinate-based figures.")


if __name__=="__main__":
    main()
