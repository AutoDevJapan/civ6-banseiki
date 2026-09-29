"""Draw the 22 civilization crests as local SVG originals and alpha PNGs.

Only a small, explicit SVG subset is emitted (polygon, circle, rect, line).
The PNG renderer reads those SVG files, so the SVG is the actual source of
each image. No downloaded image pixels or image-generation service are used.
"""

import hashlib
import json
import math
import os
from datetime import datetime, timezone
from pathlib import Path
import xml.etree.ElementTree as ET

from PIL import Image, ImageDraw

SIZE = 512
SCALE = 4
ROOT = Path(__file__).resolve().parents[2]
if os.environ.get("JAC_ART_LOCAL"):
    art_local = Path(os.environ["JAC_ART_LOCAL"]).expanduser()
    ART_LOCAL = (art_local if art_local.is_absolute() else ROOT / art_local).resolve()
else:
    ART_LOCAL = next((path.resolve() for path in
                      (ROOT.parent / "civ6-art-local", ROOT.parent.parent / "civ6-art-local")
                      if path.is_dir()), (ROOT.parent / "civ6-art-local").resolve())
OUT = ART_LOCAL / "JAC_PoC_CustomCivs" / "Originals" / "crests"
NAMES = (
    "ODA", "TAKEDA", "DATE", "ASHINA", "HOJO", "YAMANOUCHI_UESUGI",
    "NAGAO", "IMAGAWA", "MATSUDAIRA", "ASAKURA", "ASHIKAGA", "MIYOSHI",
    "ROKKAKU", "HONGANJI", "OUCHI", "AMAGO", "MORI", "CHOSOKABE",
    "OTOMO", "SHIMAZU", "RYUKYU", "AINU",
)
MOTIFS = (
    "織田木瓜", "武田菱", "竪三引両", "丸に三引両", "三つ鱗", "上杉笹",
    "九曜", "今川赤鳥", "丸に三つ葉葵", "三つ盛木瓜", "足利二つ引",
    "三階菱に五つ釘抜", "隅立て四つ目", "下り藤（1534年への適用は暫定）", "大内菱",
    "平四つ目結", "一文字三星", "七つ酢漿草", "杏葉紋", "島津十文字",
    "左三巴", "サケ（暫定・創作）",
)


class Crest:
    def __init__(self):
        self.root = ET.Element(
            "svg", {"xmlns": "http://www.w3.org/2000/svg",
                    "width": str(SIZE), "height": str(SIZE),
                    "viewBox": f"0 0 {SIZE} {SIZE}"}
        )

    def polygon(self, points, *, fill="white", stroke=None, width=1):
        attrs = {"points": " ".join(f"{x:.2f},{y:.2f}" for x, y in points),
                 "fill": fill}
        if stroke:
            attrs.update(stroke=stroke, **{"stroke-width": str(width)})
        ET.SubElement(self.root, "polygon", attrs)

    def circle(self, x, y, r, *, fill="white", stroke=None, width=1):
        attrs = {"cx": f"{x:.2f}", "cy": f"{y:.2f}", "r": f"{r:.2f}",
                 "fill": fill}
        if stroke:
            attrs.update(stroke=stroke, **{"stroke-width": str(width)})
        ET.SubElement(self.root, "circle", attrs)

    def rect(self, x, y, w, h, *, fill="white"):
        ET.SubElement(self.root, "rect", {"x": str(x), "y": str(y),
                                         "width": str(w), "height": str(h),
                                         "fill": fill})

    def line(self, x1, y1, x2, y2, width=16):
        ET.SubElement(self.root, "line",
                      {"x1": str(x1), "y1": str(y1), "x2": str(x2), "y2": str(y2),
                       "stroke": "white", "stroke-width": str(width),
                       "stroke-linecap": "round"})

    def ring(self, r=190, width=18):
        self.circle(256, 256, r, fill="none", stroke="white", width=width)

    def diamond(self, x, y, rx, ry):
        self.polygon(((x, y - ry), (x + rx, y), (x, y + ry), (x - rx, y)))

    def leaf(self, x, y, length, half_width, angle=0):
        """A pointed lens with its base at (x, y)."""
        a = math.radians(angle)
        points = []
        for side in (1, -1):
            values = range(25) if side == 1 else range(24, -1, -1)
            for i in values:
                t = i / 24
                local_x = side * half_width * math.sin(math.pi * t) ** 0.8
                local_y = -length * t
                points.append((x + local_x * math.cos(a) - local_y * math.sin(a),
                               y + local_x * math.sin(a) + local_y * math.cos(a)))
        self.polygon(points)


def rotate(points, angle, cx=256, cy=256):
    a = math.radians(angle)
    return [(cx + (x-cx)*math.cos(a) - (y-cy)*math.sin(a),
             cy + (x-cx)*math.sin(a) + (y-cy)*math.cos(a)) for x, y in points]


def cubic(start, control1, control2, end, steps=20):
    return [(
        (1-t)**3*start[0]+3*(1-t)**2*t*control1[0]+3*(1-t)*t*t*control2[0]+t**3*end[0],
        (1-t)**3*start[1]+3*(1-t)**2*t*control1[1]+3*(1-t)*t*t*control2[1]+t**3*end[1]
    ) for t in (i/steps for i in range(steps+1))]


def flower(c, x, y, r, petals=4):
    for i in range(petals):
        a = i * 360 / petals
        rad = math.radians(a)
        c.leaf(x + 0.17*r*math.sin(rad), y - 0.17*r*math.cos(rad),
               r, r*0.35, a)
    c.circle(x, y, r*0.19)


def draw(name):
    c = Crest()
    if name == "ODA":
        c.ring()
        flower(c, 256, 256, 125, petals=5)
        for a in range(5):
            rad = math.radians(a*72)
            c.circle(256+119*math.sin(rad), 256-119*math.cos(rad), 11)
    elif name == "TAKEDA":
        for x in (181, 331):
            for y in (181, 331):
                c.diamond(x, y, 75, 75)
    elif name == "DATE":
        c.ring()
        for x in (181, 256, 331):
            c.rect(x-22, 123, 44, 266)
    elif name == "ASHINA":
        c.ring()
        for y in (169, 256, 343):
            c.rect(123, y-19, 266, 38)
    elif name == "HOJO":
        for x, y in ((256, 110), (145, 298), (367, 298)):
            c.polygon(((x, y), (x-100, y+173), (x+100, y+173)))
    elif name == "YAMANOUCHI_UESUGI":
        c.ring()
        for side in (-1, 1):
            c.line(256+side*69, 370, 256+side*129, 133, 15)
            for j in range(4):
                y = 178+j*53
                c.leaf(256+side*(128-j*13), y, 77, 17, side*(55 if j%2 else -55))
            # The sparrows are deliberately reduced to a head, body, wing and tail.
            x = 256+side*54
            c.circle(x, 271, 17)
            c.leaf(x, 320, 54, 24, side*65)
            c.polygon(((x, 286), (x+side*21, 319), (x, 329), (x-side*23, 309)))
    elif name == "NAGAO":
        c.circle(256, 256, 47, fill="none", stroke="white", width=18)
        c.circle(256, 256, 15)
        for i in range(8):
            a = math.radians(i*45)
            x,y=256+126*math.sin(a),256-126*math.cos(a)
            c.circle(x,y,35,fill="none",stroke="white",width=13)
            c.circle(x,y,10)
    elif name == "IMAGAWA":
        c.ring()
        # Akadori is a stylized ceremonial comb, not a bird silhouette.
        c.circle(256,149,31)
        c.rect(128,167,256,100)
        c.polygon(((128,267),(154,299),(358,299),(384,267)))
        for x in (164,210,256,302,348):
            c.polygon(((x-13,278),(x+13,278),(x+11,381),
                       (x,398),(x-11,381)))
    elif name == "MATSUDAIRA":
        c.ring()
        for a in (0, 120, 240):
            rad = math.radians(a)
            x, y = 256+20*math.sin(rad), 256-20*math.cos(rad)
            c.leaf(x, y, 135, 49, a)
        c.circle(256, 256, 26)
    elif name == "ASAKURA":
        for x, y in ((256, 160), (164, 323), (348, 323)):
            c.circle(x, y, 82, fill="none", stroke="white", width=13)
            flower(c, x, y, 54, 4)
    elif name == "ASHIKAGA":
        for y in (201, 311):
            c.rect(93, y-23, 326, 46)
    elif name == "MIYOSHI":
        for x, y, w in ((256, 143, 58), (256, 231, 89), (256, 353, 125)):
            c.diamond(x, y, w, 37)
        for x, y in ((170,196),(342,196),(127,292),(256,292),(385,292)):
            c.diamond(x,y,20,20)
            c.polygon(((x,y-8),(x+8,y),(x,y+8),(x-8,y)),fill="black")
    elif name == "ROKKAKU":
        for x in (174, 338):
            for y in (174, 338):
                c.polygon(((x,y-73),(x+73,y),(x,y+73),(x-73,y)))
                c.polygon(((x,y-21),(x+21,y),(x,y+21),(x-21,y)),
                          fill="black")
    elif name == "HONGANJI":
        c.ring()
        c.line(140,159,256,222,14)
        c.line(372,159,256,222,14)
        for side in (-1,1):
            for j in range(4):
                x=256+side*(48+j*24)
                y=216+j*31
                c.leaf(x,y,66,18,side*(30+j*9))
            for j in range(4):
                x=256+side*(43+j*23)
                y=243+j*31
                c.leaf(x,y,54,16,side*(145-j*7))
        c.circle(256,223,20)
    elif name == "OUCHI":
        c.polygon(((256,75),(437,256),(256,437),(75,256)),
                  fill="none", stroke="white", width=20)
        for a in (0,90,180,270):
            rad=math.radians(a)
            c.leaf(256,256,121,42,a)
            c.diamond(256+111*math.sin(rad),256-111*math.cos(rad),24,24)
        c.circle(256,256,22)
    elif name == "AMAGO":
        for x in (173,339):
            for y in (173,339):
                c.rect(x-64,y-64,128,128)
                c.rect(x-19,y-19,38,38,fill="black")
    elif name == "MORI":
        c.rect(81,160,350,44)
        for x,y in ((256,264),(184,362),(328,362)):
            c.circle(x,y,42)
    elif name == "CHOSOKABE":
        c.ring()
        for i in range(7):
            a=i*360/7
            rad=math.radians(a)
            x,y=256+31*math.sin(rad),256-31*math.cos(rad)
            c.leaf(x,y,141,31,a)
        c.circle(256,256,23)
    elif name == "OTOMO":
        c.ring()
        for side in (-1,1):
            c.leaf(256+side*33,347,173,57,side*31)
            for i in range(3):
                x=256+side*(92+i*12)
                y=300-i*48
                c.leaf(x,y,66,20,side*75)
        c.circle(256,320,23)
    elif name == "SHIMAZU":
        c.ring(r=179,width=31)
        c.rect(232,119,48,274)
        c.rect(119,232,274,48)
    elif name == "RYUKYU":
        c.ring()
        # Three separate comma heads with tails turning left around the center.
        tail = (
            cubic((285,158),(313,159),(339,176),(337,204)) +
            cubic((337,204),(335,224),(317,242),(289,251))[1:] +
            cubic((289,251),(304,225),(308,205),(298,183))[1:] +
            cubic((298,183),(293,172),(286,166),(285,158))[1:]
        )
        for a in (0,120,240):
            head=rotate(((256,155),),a)[0]
            c.circle(head[0],head[1],48)
            c.polygon(rotate([(512-x,y) for x,y in tail],a))
    elif name == "AINU":
        c.ring()
        c.polygon(((106,256),(161,205),(338,202),(396,252),(337,307),
                   (162,307)))
        c.polygon(((114,256),(70,202),(70,309)))
        c.polygon(((224,211),(184,156),(282,204)))
        c.polygon(((220,299),(185,355),(286,307)))
        c.circle(346,242,8,fill="none",stroke="white",width=7)
    else:
        raise ValueError(name)
    return c


def render_svg(path):
    """Rasterize our white-only SVG primitives at 4x, then downsample alpha."""
    root = ET.parse(path).getroot()
    mask = Image.new("L", (SIZE*SCALE, SIZE*SCALE))
    d = ImageDraw.Draw(mask)
    sc = lambda value: round(float(value) * SCALE)
    for node in root:
        tag = node.tag.rsplit("}", 1)[-1]
        a = node.attrib
        fill = {"white": 255, "black": 0}.get(a.get("fill", "none"))
        outline = 255 if a.get("stroke") == "white" else None
        width = sc(a.get("stroke-width", 1))
        if tag == "polygon":
            points = [tuple(sc(v) for v in p.split(",")) for p in a["points"].split()]
            d.polygon(points, fill=fill)
            if outline:
                d.line(points+[points[0]], fill=outline, width=width, joint="curve")
        elif tag == "circle":
            x,y,r=map(float,(a["cx"],a["cy"],a["r"]))
            box=(sc(x-r),sc(y-r),sc(x+r),sc(y+r))
            d.ellipse(box,fill=fill,outline=outline,width=width)
        elif tag == "rect":
            x,y,w,h=map(float,(a["x"],a["y"],a["width"],a["height"]))
            d.rectangle((sc(x),sc(y),sc(x+w),sc(y+h)),fill=fill)
        elif tag == "line":
            xy=[sc(a[k]) for k in ("x1","y1","x2","y2")]
            d.line(xy,fill=outline,width=width)
            radius=width//2
            for x,y in ((xy[0],xy[1]),(xy[2],xy[3])):
                d.ellipse((x-radius,y-radius,x+radius,y+radius),fill=outline)
        else:
            raise ValueError(f"unsupported SVG element: {tag}")
    alpha=mask.resize((SIZE,SIZE),Image.Resampling.LANCZOS)
    image=Image.new("RGBA",(SIZE,SIZE),"white")
    image.putalpha(alpha)
    return image


def generate():
    OUT.mkdir(parents=True, exist_ok=True)
    records=[]
    for name,motif in zip(NAMES,MOTIFS,strict=True):
        svg=OUT/f"{name.lower()}.svg"
        png=OUT/f"{name.lower()}.png"
        ET.indent((crest:=draw(name)).root)
        ET.ElementTree(crest.root).write(svg,encoding="utf-8",xml_declaration=True)
        image=render_svg(svg)
        image.save(png)
        records.append({
            "civilization":f"CIVILIZATION_JAC_{name}",
            "motif":motif,"method":"original geometric SVG, rasterized by draw_crests.py",
            "author":"Civ6用万世紀制作チーム","created_utc":datetime.now(timezone.utc).isoformat(),
            "svg":svg.name,"png":png.name,
            "svg_sha256":hashlib.sha256(svg.read_bytes()).hexdigest(),
            "png_sha256":hashlib.sha256(png.read_bytes()).hexdigest(),
        })
    (OUT/"generation-log.json").write_text(
        json.dumps(records,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(f"Generated {len(records)} SVG + PNG crests in {OUT}")


if __name__ == "__main__":
    generate()
