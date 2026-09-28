"""JAC_PoC_CustomCivs の指導者・文明の画像を作り、SDK の Cooker で BLP にする。

使い方: python tools/jac_art/build_art.py
入力: poc/JAC_PoC_CustomCivs/ArtSource/Originals/ の元画像（出典は documents/poc/README.md）
出力: poc/JAC_PoC_CustomCivs/Platforms/Windows/BLPs/ の BLP 3つと SHARED_DATA/
中間ファイル（.dds / .tex / .xlp）は tools/jac_art/build/ に作る（Git 管理外）。
"""
import math
import shutil
import struct
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parents[2]
MOD = ROOT / "poc" / "JAC_PoC_CustomCivs"
ORIGINALS = MOD / "ArtSource" / "Originals"
BUILD = Path(__file__).resolve().parent / "build"
STEWPOT = MOD / "Platforms" / "Windows" / "BLPs"
COOKER_DIR = Path(r"C:\Program Files (x86)\Steam\steamapps\common\Sid Meier's Civilization VI SDK\AssetModTools\Cooker")

RED_DK = (120, 0, 1, 255)  # COLOR_STANDARD_RED_DK
WHITE_LT = (249, 249, 249, 255)  # COLOR_STANDARD_WHITE_LT

LEADER_ICON_SIZES = (32, 45, 48, 50, 55, 64, 80, 256)  # XP2 の ICON_ATLAS_LEADERS と同じ
CIV_ICON_SIZES = (22, 30, 32, 36, 44, 45, 48, 50, 64, 80, 128, 200, 256)  # XP2 の文明アイコンと同じ
PORTRAIT_SIZE = (570, 1024)
BACKGROUND_SIZE = (1920, 1080)  # LoadScreen.xml の帯（高さ987）が切れず、1080p で上下に黒帯が出ない

LEADERS = {
    # 0番: 織田。萬松寺の木像写真。帽子の上に余白が無いので上を足す。
    "ODA_NOBUHIDE": {
        "file": "oda_nobuhide_statue.jpg",
        "portrait": (478, 0, 1278, 1300),
        "portrait_pad_top": 137,
        "face": (608, 60, 1148, 600),
        "background": ("owari_fujimigahara.jpg", (98, 178, 3826, 2275)),
    },
    # 1番: 武田。大泉寺の肖像画。
    "TAKEDA_NOBUTORA": {
        "file": "takeda_nobutora.jpg",
        "portrait": (0, 200, 358, 843),
        "portrait_pad_top": 0,
        "face": (57, 300, 287, 530),
        "background": ("kai_saruhashi.jpg", (887, 1121, 2103, 1805)),
    },
}


def write_dds(path, im):
    # Pillow の DDS 保存は B,G,R,A 順で、Cooker はマスクを見ずにそのまま使うため赤青が入れ替わる。
    # R,G,B,A 順の非圧縮 DDS ヘッダーを自前で書く。
    w, h = im.size
    hdr = struct.pack("<4sIIIIIII44sIIIIIIIIIIIII", b"DDS ", 124, 0x100F, h, w, w * 4, 0, 1, b"\0" * 44,
                      32, 0x41, 0, 32, 0x000000FF, 0x0000FF00, 0x00FF0000, 0xFF000000, 0x1000, 0, 0, 0, 0)
    path.write_bytes(hdr + im.convert("RGBA").tobytes())


def write_tex(name, size, cls):
    tags = "".join(f'\n\t\t<Element text="{t}"/>' for t in ([cls, "Leader", "Fallback"] if cls == "Leader_Fallback" else [cls]))
    (BUILD / "Textures" / f"{name}.tex").write_text(f"""<?xml version="1.0" encoding="UTF-8" ?>
<AssetObjects..TextureInstance>
	<m_ExportSettings>
		<ePixelformat>PF_R8G8B8A8_UNORM</ePixelformat>
		<eFilterType>FT_LANCZOS6</eFilterType>
		<bUseMips>false</bUseMips>
		<iNumManualMips>0</iNumManualMips>
		<bCompleteMipChain>false</bCompleteMipChain>
		<fValueClampMin>0.000000</fValueClampMin>
		<fValueClampMax>1.000000</fValueClampMax>
		<fSupportScale>1.000000</fSupportScale>
		<fGammaIn>2.200000</fGammaIn>
		<fGammaOut>2.200000</fGammaOut>
		<iSlabWidth>0</iSlabWidth>
		<iSlabHeight>0</iSlabHeight>
		<iColorKeyX>64</iColorKeyX>
		<iColorKeyY>64</iColorKeyY>
		<iColorKeyZ>64</iColorKeyZ>
		<eExportMode>TEXTURE_2D</eExportMode>
		<bSampleFromTopLayer>false</bSampleFromTopLayer>
	</m_ExportSettings>
	<m_CookParams>
		<m_Values/>
	</m_CookParams>
	<m_Version>
		<major>1</major>
		<minor>0</minor>
		<build>0</build>
		<revision>0</revision>
	</m_Version>
	<m_Height>{size[1]}</m_Height>
	<m_Width>{size[0]}</m_Width>
	<m_Depth>1</m_Depth>
	<m_NumMipMaps>0</m_NumMipMaps>
	<m_SourceFilePath text="{name}.dds"/>
	<m_SourceObjectName text=""/>
	<m_ImportedTime>0</m_ImportedTime>
	<m_ExportedTime>0</m_ExportedTime>
	<m_ClassName text="{cls}"/>
	<m_DataFiles>
		<Element>
			<m_ID text="DDS"/>
			<m_RelativePath text="{name}.dds"/>
		</Element>
	</m_DataFiles>
	<m_Name text="{name}"/>
	<m_Description text=""/>
	<m_Tags>{tags}
	</m_Tags>
	<m_Groups/>
</AssetObjects..TextureInstance>
""", encoding="utf-8")


def write_xlp(package, cls, names):
    entries = "".join(f"""
		<Element>
			<m_EntryID text="{n}"/>
			<m_ObjectName text="{n}"/>
		</Element>""" for n in names)
    (BUILD / "XLPs" / f"{package}.xlp").write_text(f"""<?xml version="1.0" encoding="UTF-8" ?>
<AssetObjects..XLP>
	<m_Version>
		<major>4</major>
		<minor>0</minor>
		<build>253</build>
		<revision>293</revision>
	</m_Version>
	<m_ClassName text="{cls}"/>
	<m_PackageName text="{package}"/>
	<m_Entries>{entries}
	</m_Entries>
	<m_AllowedPlatforms>
		<Element>WINDOWS</Element>
	</m_AllowedPlatforms>
</AssetObjects..XLP>
""", encoding="utf-8")


def texture(name, im, cls="UserInterface"):
    write_dds(BUILD / "Textures" / f"{name}.dds", im)
    write_tex(name, im.size, cls)
    return name


def aa_mask(size, draw_fn, scale=4):
    """draw_fn(draw, w, h) で白く塗った図形を scale 倍で描いて縮小したアンチエイリアス付きマスクにする。"""
    big = Image.new("L", (size[0] * scale, size[1] * scale), 0)
    draw_fn(ImageDraw.Draw(big), big.width, big.height)
    return big.resize(size, Image.LANCZOS)


def portrait(key):
    info = LEADERS[key]
    src = Image.open(ORIGINALS / info["file"]).convert("RGB")
    crop = src.crop(info["portrait"])
    pad = info["portrait_pad_top"]
    if pad:
        # 上の余白は元画像の上端の色で埋める（縁はぼかして透明にするので継ぎ目は見えない）。
        canvas = Image.new("RGB", (crop.width, crop.height + pad), crop.getpixel((crop.width // 2, 2)))
        canvas.paste(crop, (0, pad))
        crop = canvas
    im = crop.resize(PORTRAIT_SIZE, Image.LANCZOS).convert("RGBA")
    # 縁を透明にぼかし、ロード画面・外交画面の背景になじませる。
    w, h = PORTRAIT_SIZE
    fade = Image.new("L", PORTRAIT_SIZE, 0)
    ImageDraw.Draw(fade).rounded_rectangle((40, 40, w - 40, h - 40), radius=60, fill=255)
    im.putalpha(fade.filter(ImageFilter.GaussianBlur(28)))
    return im


def background(key):
    file, box = LEADERS[key]["background"]
    return Image.open(ORIGINALS / file).convert("RGB").crop(box).resize(BACKGROUND_SIZE, Image.LANCZOS).convert("RGBA")


def leader_icon_master(key, size=512):
    face = Image.open(ORIGINALS / LEADERS[key]["file"]).convert("RGB").crop(LEADERS[key]["face"])
    im = face.resize((size, size), Image.LANCZOS).convert("RGBA")
    im.putalpha(aa_mask((size, size), lambda d, w, h: d.ellipse((0, 0, w - 1, h - 1), fill=255)))
    return im


def lobes(d, w, cx, cy, r, dist, count=5, fill=255):
    for k in range(count):
        a = math.radians(-90 + k * 360 / count)
        x, y = cx + dist * w * math.cos(a), cy + dist * w * math.sin(a)
        d.ellipse((x - r * w, y - r * w, x + r * w, y + r * w), fill=fill)


def draw_oda_mokko(d, w, h):
    """織田木瓜（五つ木瓜）を単純化したもの：外側の帯、内側の帯、中央の花。"""
    cx, cy = w / 2, h / 2
    lobes(d, w, cx, cy, 0.20, 0.25)
    d.ellipse((cx - 0.25 * w, cy - 0.25 * w, cx + 0.25 * w, cy + 0.25 * w), fill=255)
    lobes(d, w, cx, cy, 0.160, 0.25, fill=0)
    d.ellipse((cx - 0.22 * w, cy - 0.22 * w, cx + 0.22 * w, cy + 0.22 * w), fill=0)
    lobes(d, w, cx, cy, 0.105, 0.15)
    lobes(d, w, cx, cy, 0.070, 0.15, fill=0)
    d.ellipse((cx - 0.09 * w, cy - 0.09 * w, cx + 0.09 * w, cy + 0.09 * w), fill=0)
    lobes(d, w, cx, cy, 0.045, 0.055)
    d.ellipse((cx - 0.02 * w, cy - 0.02 * w, cx + 0.02 * w, cy + 0.02 * w), fill=0)


def draw_takeda_bishi(d, w, h):
    """武田菱（四つ割菱）：大きな菱を十字に割った4つの菱。"""
    cx, cy = w / 2, h / 2
    a, b, gap = 0.44 * w, 0.34 * h, 0.035 * w
    for ox, oy in ((0, -b / 2), (0, b / 2), (-a / 2, 0), (a / 2, 0)):
        sa, sb = a / 2 - gap, b / 2 - gap * b / a
        x, y = cx + ox, cy + oy
        d.polygon([(x - sa, y), (x, y - sb), (x + sa, y), (x, y + sb)], fill=255)


def civ_icon_master(draw_fn, colored, fg, bg, size=1024):
    mon = aa_mask((size, size), draw_fn, scale=1)
    if not colored:
        # 通常サイズは白い図形＋透明背景（UI 側で色を付ける）。
        im = Image.new("RGBA", (size, size), (255, 255, 255, 0))
        im.putalpha(mon)
        return im
    # 45px だけは色付き（公式の CivDefaults45 と同じ扱い）。
    circle = aa_mask((size, size), lambda d, w, h: d.ellipse((0, 0, w - 1, h - 1), fill=255), scale=1)
    im = Image.new("RGBA", (size, size), bg)
    im.paste(Image.new("RGBA", (size, size), fg), (0, 0), mon)
    im.putalpha(circle)
    return im


def atlas(masters, size):
    """2列×1行のアトラス（0番=織田、1番=武田）。"""
    out = Image.new("RGBA", (size * 2, size), (0, 0, 0, 0))
    for i, m in enumerate(masters):
        out.paste(m.resize((size, size), Image.LANCZOS), (i * size, 0))
    return out


def cook(package):
    cmd = [str(COOKER_DIR / "Civ6AssetCooker_FinalRelease.exe"), "--no_mt", "--mode", "XLP", "--platform", "Windows",
           "--pantry", str(BUILD), "--stewpot", str(STEWPOT), "--config", str(COOKER_DIR / "Civ6.cfg"),
           str(BUILD / "XLPs" / f"{package}.xlp")]
    result = subprocess.run(cmd, capture_output=True, text=True, errors="replace")
    print(result.stdout[-2000:], result.stderr[-2000:], sep="\n")
    if result.returncode != 0:
        sys.exit(f"Cooker failed for {package}: exit {result.returncode}")


def main():
    shutil.rmtree(BUILD, ignore_errors=True)
    (BUILD / "Textures").mkdir(parents=True)
    (BUILD / "XLPs").mkdir()
    STEWPOT.mkdir(parents=True, exist_ok=True)

    order = ("ODA_NOBUHIDE", "TAKEDA_NOBUTORA")
    leader_masters = [leader_icon_master(k) for k in order]
    civ_draw = (draw_oda_mokko, draw_takeda_bishi)
    civ_colors = ((RED_DK, WHITE_LT), (WHITE_LT, RED_DK))  # (図形, 背景) = 各家の PlayerColors の (副色, 主色)
    icons = [texture(f"JAC_Leaders{s}", atlas(leader_masters, s)) for s in LEADER_ICON_SIZES]
    for s in CIV_ICON_SIZES:
        masters = [civ_icon_master(fn, s == 45, fg, bg) for fn, (fg, bg) in zip(civ_draw, civ_colors)]
        icons.append(texture(f"JAC_Civs{s}", atlas(masters, s)))

    leaders, fallbacks = [], []
    for key in order:
        p = portrait(key)
        leaders.append(texture(f"LEADER_JAC_{key}_NEUTRAL", p))
        leaders.append(texture(f"LEADER_JAC_{key}_BACKGROUND", background(key)))
        fallbacks.append(texture(f"FALLBACK_NEUTRAL_JAC_{key}", p, "Leader_Fallback"))

    packages = {
        "JAC_CustomCivs_Icons": ("UITexture", icons),
        "JAC_CustomCivs_Leaders": ("UITexture", leaders),
        "JAC_CustomCivs_Fallback": ("LeaderFallback", fallbacks),
    }
    for package, (cls, names) in packages.items():
        write_xlp(package, cls, names)
        cook(package)

    # 最低限の確認：BLP ができていて、各テクスチャ名が中に入っている。
    for package, (_, names) in packages.items():
        blp = (STEWPOT / f"{package}.blp").read_bytes()
        missing = [n for n in names if n.encode() not in blp]
        assert not missing, f"{package}.blp lacks {missing}"
        print(f"OK {package}.blp {len(blp)} bytes, {len(names)} textures")


if __name__ == "__main__":
    main()
