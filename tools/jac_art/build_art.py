"""Build CustomCivs art, and GrowthUI art when its mod is present.

Usage: python tools/jac_art/build_art.py
Optional: JAC_ART_LOCAL=<art store>, JAC_GROWTH_MOD=<GrowthUI mod directory>
"""
import hashlib
import os
import shutil
import struct
import subprocess
import sys
import time
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageOps

ROOT = Path(__file__).resolve().parents[2]
MOD = ROOT / "poc" / "JAC_PoC_CustomCivs"
BUILD = Path(__file__).resolve().parent / "build"
STEWPOT = MOD / "Platforms" / "Windows" / "BLPs"
COOKER_DIR = Path(r"C:\Program Files (x86)\Steam\steamapps\common\Sid Meier's Civilization VI SDK\AssetModTools\Cooker")

LEADER_ICON_SIZES = (32, 45, 48, 50, 55, 64, 80, 256)  # XP2 の ICON_ATLAS_LEADERS と同じ
CIV_ICON_SIZES = (22, 30, 32, 36, 44, 45, 48, 50, 64, 80, 128, 200, 256)  # XP2 の文明アイコンと同じ
ABILITY_ICON_SIZES = CIV_ICON_SIZES
PORTRAIT_SIZE = (570, 1024)
BACKGROUND_SIZE = (1920, 1080)  # LoadScreen.xml は画面全体に表示
DIPLOMACY_BACKGROUND_SIZE = (1600, 1000)  # LeaderScene.xml は画面全体へ Fill。実機の約16:10に合わせる


def local_art_root():
    override = os.environ.get("JAC_ART_LOCAL")
    if override:
        path = Path(override).expanduser()
        return (path if path.is_absolute() else ROOT / path).resolve()
    # Prefer ../civ6-art-local; this multi-worktree checkout uses ../../civ6-art-local.
    for path in (ROOT.parent / "civ6-art-local", ROOT.parent.parent / "civ6-art-local"):
        if path.is_dir():
            return path.resolve()
    return (ROOT.parent / "civ6-art-local").resolve()


ART_LOCAL = local_art_root()
ORIGINALS = ART_LOCAL / "JAC_PoC_CustomCivs" / "Originals"
GROWTH_ORIGINALS = ART_LOCAL / "JAC_PoC_GrowthUI" / "Originals"
FINAL = ART_LOCAL / "final-01"
GROWTH_MOD = Path(os.environ.get("JAC_GROWTH_MOD", str(ROOT / "poc" / "JAC_PoC_GrowthUI"))).expanduser().resolve()

LEADERS = {
    "ODA_NOBUHIDE": {
        "portrait": "oda_nobuhide_portrait.jpg",
        "figure": "oda_nobuhide_figure.png",
        "background": "oda_nobuhide_background.png",
        "icon_crop": (235, 20, 815, 600),
    },
    "TAKEDA_NOBUTORA": {
        "portrait": "takeda_nobutora_portrait.jpg",
        "figure": "takeda_nobutora_figure.png",
        "background": "takeda_nobutora_background.png",
        "icon_crop": (230, 25, 810, 605),
    },
}
ABILITY_SOURCES = (
    "oda_clan_ability_generated.png",
    "takeda_clan_ability_generated_v2.png",
    "oda_nobuhide_ability_generated.png",
    "takeda_nobutora_ability_generated.png",
)


def write_dds(path, im):
    # Pillow の DDS 保存は B,G,R,A 順で、Cooker はマスクを見ずにそのまま使うため赤青が入れ替わる。
    # R,G,B,A 順の非圧縮 DDS ヘッダーを自前で書く。
    w, h = im.size
    hdr = struct.pack("<4sIIIIIII44sIIIIIIIIIIIII", b"DDS ", 124, 0x100F, h, w, w * 4, 0, 1, b"\0" * 44,
                      32, 0x41, 0, 32, 0x000000FF, 0x0000FF00, 0x00FF0000, 0xFF000000, 0x1000, 0, 0, 0, 0)
    path.parent.mkdir(parents=True, exist_ok=True)
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
    im.save(FINAL / f"{name}.png")
    return name


def key_magenta(path):
    """Extract a generated figure or UI icon from its flat magenta background."""
    rgb = Image.open(path).convert("RGB")
    r, g, b = rgb.split()
    distance = ImageChops.lighter(ImageChops.lighter(ImageOps.invert(r), g), ImageOps.invert(b))
    alpha = distance.point([max(0, min(255, round((value - 18) * 255 / 75))) for value in range(256)])
    magenta = ImageChops.subtract(ImageChops.darker(r, b), g)
    hue_alpha = magenta.point([max(0, min(255, round((100 - value) * 255 / 65))) for value in range(256)])
    alpha = ImageChops.darker(alpha, hue_alpha).filter(ImageFilter.MinFilter(5))
    out = rgb.convert("RGBA")
    out.putalpha(alpha)
    return out


def foreground(key, max_height):
    keyed = key_magenta(ORIGINALS / LEADERS[key]["figure"])
    bounds = keyed.getchannel("A").getbbox()
    if not bounds:
        raise ValueError(f"Empty keyed figure: {key}")
    cut = keyed.crop(bounds)
    cut.thumbnail((550, max_height), Image.Resampling.LANCZOS)
    out = Image.new("RGBA", PORTRAIT_SIZE)
    out.alpha_composite(cut, ((out.width - cut.width) // 2, out.height - cut.height))
    return out


def background(key, size):
    src = Image.open(ORIGINALS / LEADERS[key]["background"]).convert("RGB")
    return ImageOps.fit(src, size, method=Image.Resampling.LANCZOS).convert("RGBA")


def leader_icon_master(key):
    src = Image.open(ORIGINALS / LEADERS[key]["portrait"]).convert("RGB")
    face = src.crop(LEADERS[key]["icon_crop"])
    size = 512
    im = face.resize((size, size), Image.Resampling.LANCZOS).convert("RGBA")
    big = Image.new("L", (size * 4, size * 4))
    ImageDraw.Draw(big).ellipse((0, 0, size * 4 - 1, size * 4 - 1), fill=255)
    im.putalpha(big.resize((size, size), Image.Resampling.LANCZOS))
    return im


def symbol_icon_master(filename):
    gray = ImageOps.grayscale(Image.open(ORIGINALS / filename).convert("RGB"))
    # Generated black-on-white mon -> white symbol with transparent background.
    alpha = gray.point([max(0, min(255, round((220 - value) * 255 / 170))) for value in range(256)])
    bounds = alpha.point(lambda value: 255 if value > 100 else 0).getbbox()
    if not bounds:
        raise ValueError(f"Empty generated mon: {filename}")
    symbol = alpha.crop(bounds)
    symbol.thumbnail((460, 460), Image.Resampling.LANCZOS)
    centered = Image.new("L", (512, 512))
    centered.paste(symbol, ((512 - symbol.width) // 2, (512 - symbol.height) // 2))
    white = Image.new("RGBA", (512, 512), "white")
    white.putalpha(centered)
    return white


def atlas(masters, size):
    """Single-row atlas in source order."""
    out = Image.new("RGBA", (size * len(masters), size), (0, 0, 0, 0))
    for i, m in enumerate(masters):
        out.alpha_composite(m.resize((size, size), Image.Resampling.LANCZOS), (i * size, 0))
    return out


def growth_icon():
    keyed = key_magenta(GROWTH_ORIGINALS / "notebooks_generated.png")
    bounds = keyed.getchannel("A").getbbox()
    if not bounds:
        raise ValueError("Empty notebooks icon")
    cut = keyed.crop(bounds)
    cut.thumbnail((54, 54), Image.Resampling.LANCZOS)
    out = Image.new("RGBA", (64, 64))
    out.alpha_composite(cut, ((64 - cut.width) // 2, (64 - cut.height) // 2))
    dest = GROWTH_MOD / "UI" / "Textures" / "JAC_Notebooks_Hook.dds"
    write_dds(dest, out)
    out.save(FINAL / "JAC_Notebooks_Hook.png")
    print(f"OK {dest} {dest.stat().st_size} bytes")


def contact_sheet(include_growth):
    names = [f"JAC_Leaders{size}" for size in LEADER_ICON_SIZES]
    names += [f"JAC_Civs{size}" for size in CIV_ICON_SIZES]
    names += [f"JAC_Abilities{size}" for size in ABILITY_ICON_SIZES]
    for key in LEADERS:
        names += [f"LEADER_JAC_{key}_NEUTRAL", f"LEADER_JAC_{key}_BACKGROUND",
                  f"LEADER_JAC_{key}_DIPLOMACY_BACKGROUND",
                  f"FALLBACK_NEUTRAL_JAC_{key}"]
    if include_growth:
        names.append("JAC_Notebooks_Hook")
    pngs = [FINAL / f"{name}.png" for name in names]
    cols, cell_w, cell_h = 4, 420, 320
    rows = (len(pngs) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * cell_w, rows * cell_h), (34, 39, 48))
    draw = ImageDraw.Draw(sheet)
    for index, path in enumerate(pngs):
        x, y = (index % cols) * cell_w, (index // cols) * cell_h
        draw.rounded_rectangle((x + 8, y + 8, x + cell_w - 8, y + cell_h - 8), radius=10, fill=(65, 72, 82))
        draw.text((x + 18, y + 15), path.stem, fill="white")
        image = Image.open(path).convert("RGBA")
        image.thumbnail((cell_w - 36, cell_h - 55), Image.Resampling.LANCZOS)
        pane = Image.new("RGBA", (cell_w - 36, cell_h - 55), (115, 123, 133, 255))
        pane.alpha_composite(image, ((pane.width - image.width) // 2, (pane.height - image.height) // 2))
        sheet.paste(pane.convert("RGB"), (x + 18, y + 43))
    sheet.save(FINAL / "contact-sheet.png")
    print(f"OK {FINAL / 'contact-sheet.png'}: {len(pngs)} final images")


def cook(package):
    cmd = [str(COOKER_DIR / "Civ6AssetCooker_FinalRelease.exe"), "--no_mt", "--mode", "XLP", "--platform", "Windows",
           "--pantry", str(BUILD), "--stewpot", str(STEWPOT), "--config", str(COOKER_DIR / "Civ6.cfg"),
           str(BUILD / "XLPs" / f"{package}.xlp")]
    result = subprocess.run(cmd, cwd=BUILD, capture_output=True, text=True, errors="replace")
    print(result.stdout[-2000:], result.stderr[-2000:], sep="\n")
    if result.returncode != 0:
        sys.exit(f"Cooker failed for {package}: exit {result.returncode}")


def verify_blp(package, names, started_ns):
    path = STEWPOT / f"{package}.blp"
    if not path.is_file():
        raise RuntimeError(f"Cooker did not create {path}")
    stat = path.stat()
    if stat.st_size < 1024 or stat.st_mtime_ns < started_ns:
        raise RuntimeError(f"Cooker output is empty or stale: {path} ({stat.st_size} bytes, mtime={stat.st_mtime_ns})")
    content = path.read_bytes()
    missing = [name for name in names if name.encode("ascii") not in content]
    if missing:
        raise RuntimeError(f"{path} lacks {missing}")
    digest = hashlib.sha256(content).hexdigest()
    print(f"OK {path.name} {stat.st_size} bytes, {len(names)} textures, mtime_ns={stat.st_mtime_ns}, sha256={digest}")


def main():
    growth_available = (GROWTH_MOD / "JAC_PoC_GrowthUI.modinfo").is_file()
    required = [ORIGINALS / value for leader in LEADERS.values()
                for value in (leader["portrait"], leader["figure"], leader["background"])]
    required += [ORIGINALS / "oda_mokko_generated.png", ORIGINALS / "takeda_bishi_generated.png"]
    if growth_available:
        required.append(GROWTH_ORIGINALS / "notebooks_generated.png")
    required += [ORIGINALS / name for name in ABILITY_SOURCES]
    missing = [str(path) for path in required if not path.is_file()]
    if missing:
        sys.exit("Missing local originals:\n" + "\n".join(missing))
    if not (COOKER_DIR / "Civ6AssetCooker_FinalRelease.exe").is_file():
        sys.exit(f"Missing Civ6 SDK Asset Cooker: {COOKER_DIR}")
    if not growth_available:
        print(f"SKIP GrowthUI DDS: mod folder absent at {GROWTH_MOD} (set JAC_GROWTH_MOD to override)")

    shutil.rmtree(BUILD, ignore_errors=True)
    (BUILD / "Textures").mkdir(parents=True)
    (BUILD / "XLPs").mkdir()
    STEWPOT.mkdir(parents=True, exist_ok=True)
    FINAL.mkdir(parents=True, exist_ok=True)

    order = ("ODA_NOBUHIDE", "TAKEDA_NOBUTORA")
    leader_masters = [leader_icon_master(k) for k in order]
    civ_masters = [symbol_icon_master(name) for name in ("oda_mokko_generated.png", "takeda_bishi_generated.png")]
    ability_masters = [symbol_icon_master(name) for name in ABILITY_SOURCES]
    icons = [texture(f"JAC_Leaders{s}", atlas(leader_masters, s)) for s in LEADER_ICON_SIZES]
    for s in CIV_ICON_SIZES:
        icons.append(texture(f"JAC_Civs{s}", atlas(civ_masters, s)))
    for s in ABILITY_ICON_SIZES:
        icons.append(texture(f"JAC_Abilities{s}", atlas(ability_masters, s)))

    leaders, fallbacks = [], []
    for key in order:
        # LoadScreen は肖像を上端で切り取るため、髷の上に余白を確保する。
        p = foreground(key, 860)
        leaders.append(texture(f"LEADER_JAC_{key}_NEUTRAL", p))
        leaders.append(texture(f"LEADER_JAC_{key}_BACKGROUND", background(key, BACKGROUND_SIZE)))
        leaders.append(texture(f"LEADER_JAC_{key}_DIPLOMACY_BACKGROUND", background(key, DIPLOMACY_BACKGROUND_SIZE)))
        fallbacks.append(texture(f"FALLBACK_NEUTRAL_JAC_{key}", foreground(key, 1010), "Leader_Fallback"))

    packages = {
        "JAC_CustomCivs_Icons": ("UITexture", icons),
        "JAC_CustomCivs_Leaders": ("UITexture", leaders),
        "JAC_CustomCivs_Fallback": ("LeaderFallback", fallbacks),
    }
    for package in packages:
        (STEWPOT / f"{package}.blp").unlink(missing_ok=True)
    for package, (cls, names) in packages.items():
        write_xlp(package, cls, names)
        started_ns = time.time_ns()
        cook(package)
        verify_blp(package, names, started_ns)

    if growth_available:
        growth_icon()
    contact_sheet(growth_available)


if __name__ == "__main__":
    main()
