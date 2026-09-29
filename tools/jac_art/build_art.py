"""Build CustomCivs art, and GrowthUI art when its mod is present.

Usage: python tools/jac_art/build_art.py
Optional: JAC_ART_LOCAL=<art store>, JAC_GROWTH_MOD=<GrowthUI mod directory>,
          JAC_COOKER_DIR=<Civ6 SDK AssetModTools/Cooker directory>
"""
import hashlib
import math
import os
import shutil
import struct
import subprocess
import sys
import time
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageOps

from draw_crests import NAMES as CREST_NAMES

ROOT = Path(__file__).resolve().parents[2]
MOD = ROOT / "poc" / "JAC_PoC_CustomCivs"
BUILD = Path(__file__).resolve().parent / "build"
STAGING = BUILD / "cooked"
STEWPOT = MOD / "Platforms" / "Windows" / "BLPs"
COOKER_DIR = Path(os.environ.get("JAC_COOKER_DIR", r"C:\Program Files (x86)\Steam\steamapps\common\Sid Meier's Civilization VI SDK\AssetModTools\Cooker")).expanduser().resolve()

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
PREVIEWS = BUILD / "previews"
GROWTH_MOD = Path(os.environ.get("JAC_GROWTH_MOD", str(ROOT / "poc" / "JAC_PoC_GrowthUI"))).expanduser().resolve()

# Keep this order aligned with Config/JAC_Japan1534_Players.xml's icon indices.
LEADER_ORDER = (
    "ODA_NOBUHIDE", "TAKEDA_NOBUTORA", "DATE_TANEMUNE", "ASHINA_MORIKIYO",
    "HOJO_UJITSUNA", "YAMANOUCHI_UESUGI_NORIMASA", "NAGAO_TAMEKAGE",
    "IMAGAWA_UJITERU", "MATSUDAIRA_KIYOYASU", "ASAKURA_TAKAKAGE",
    "ASHIKAGA_YOSHIHARU", "MIYOSHI_NAGAYOSHI", "ROKKAKU_SADAYORI",
    "HONGANJI_SHONYO", "OUCHI_YOSHITAKA", "AMAGO_TSUNEHISA",
    "MORI_MOTONARI", "CHOSOKABE_KUNICHIKA", "OTOMO_YOSHIAKI",
    "SHIMAZU_TAKAHISA", "RYUKYU_SHO_SEI", "AINU_TARIKONA",
)
LEADERS = {
    key: {
        kind: f"{key.lower()}/{key.lower()}_{kind}_codex.png"
        for kind in ("portrait", "figure", "background")
    }
    for key in LEADER_ORDER
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
    im.save(PREVIEWS / f"{name}.png")
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
    edge = min(src.size)
    face = ImageOps.fit(src, (edge, edge), method=Image.Resampling.LANCZOS)
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


def atlas(masters, size, per_row=None):
    """Atlas in source order, with optional row width for many leaders."""
    per_row = per_row or len(masters)
    out = Image.new("RGBA", (size * per_row, size * math.ceil(len(masters) / per_row)), (0, 0, 0, 0))
    for i, m in enumerate(masters):
        out.alpha_composite(m.resize((size, size), Image.Resampling.LANCZOS),
                            ((i % per_row) * size, (i // per_row) * size))
    return out


def grid_atlas(masters, size, columns=8):
    """Rows follow icon index order, as in XP2 civilization atlases."""
    rows = (len(masters) + columns - 1) // columns
    out = Image.new("RGBA", (columns * size, rows * size), (0, 0, 0, 0))
    for i, master in enumerate(masters):
        out.alpha_composite(master.resize((size, size), Image.Resampling.LANCZOS),
                            ((i % columns) * size, (i // columns) * size))
    return out


def crest_masters():
    masters = []
    for name in CREST_NAMES:
        master = Image.open(ORIGINALS / "crests" / f"{name.lower()}.png").convert("RGBA")
        if master.size != (512, 512) or master.getchannel("A").getbbox() is None:
            raise ValueError(f"Invalid crest image: {name}")
        masters.append(master)
    return masters


def crest_contact_sheet(masters):
    """Readable local proof with full-size and 22px samples for each house."""
    cell_w, cell_h, columns = 180, 195, 6
    rows = (len(masters) + columns - 1) // columns
    sheet = Image.new("RGBA", (columns * cell_w, rows * cell_h), "#45484d")
    draw = ImageDraw.Draw(sheet)
    for i, (name, master) in enumerate(zip(CREST_NAMES, masters, strict=True)):
        x, y = (i % columns) * cell_w, (i // columns) * cell_h
        sheet.alpha_composite(master.resize((104, 104), Image.Resampling.LANCZOS),
                              (x + 22, y + 12))
        sheet.alpha_composite(master.resize((22, 22), Image.Resampling.LANCZOS),
                              (x + 135, y + 60))
        draw.text((x + 8, y + 143), name, fill="white")
    sheet.convert("RGB").save(PREVIEWS / "house-crests.png")


def growth_icon():
    keyed = key_magenta(GROWTH_ORIGINALS / "notebooks_generated.png")
    bounds = keyed.getchannel("A").getbbox()
    if not bounds:
        raise ValueError("Empty notebooks icon")
    cut = keyed.crop(bounds)
    # PartialScreenHooks.xml renders this as a 26x26 Image with no sprite offset.
    # Keep the artwork within that frame so the engine does not clip a 64x64 source.
    cut.thumbnail((24, 24), Image.Resampling.LANCZOS)
    out = Image.new("RGBA", (26, 26), (0, 0, 0, 0))
    out.alpha_composite(cut, ((26 - cut.width) // 2, (26 - cut.height) // 2))
    dest = GROWTH_MOD / "UI" / "Textures" / "JAC_Notebooks_Hook.dds"
    write_dds(dest, out)
    out.save(PREVIEWS / "JAC_Notebooks_Hook.png")
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
    pngs = [PREVIEWS / f"{name}.png" for name in names]
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
    sheet.save(PREVIEWS / "contact-sheet.png")
    print(f"OK {PREVIEWS / 'contact-sheet.png'}: {len(pngs)} final images")

    # Keep a readable face sheet: the 8x3 atlas is too small in the full sheet.
    face_sheet = Image.new("RGB", (4 * 260, math.ceil(len(LEADER_ORDER) / 4) * 290), (34, 39, 48))
    face_draw = ImageDraw.Draw(face_sheet)
    for index, key in enumerate(LEADER_ORDER):
        x, y = (index % 4) * 260, (index // 4) * 290
        face = leader_icon_master(key).resize((220, 220), Image.Resampling.LANCZOS)
        tile = Image.new("RGBA", (220, 220), (115, 123, 133, 255))
        tile.alpha_composite(face)
        face_sheet.paste(tile.convert("RGB"), (x + 20, y + 18))
        face_draw.text((x + 12, y + 248), key, fill="white")
    face_sheet.save(PREVIEWS / "portrait-sheet.png")
    print(f"OK {PREVIEWS / 'portrait-sheet.png'}: {len(LEADER_ORDER)} faces")


def cook(package, stewpot):
    cmd = [str(COOKER_DIR / "Civ6AssetCooker_FinalRelease.exe"), "--no_mt", "--mode", "XLP", "--platform", "Windows",
           "--pantry", str(BUILD), "--stewpot", str(stewpot), "--config", str(COOKER_DIR / "Civ6.cfg"),
           str(BUILD / "XLPs" / f"{package}.xlp")]
    result = subprocess.run(cmd, cwd=BUILD, capture_output=True, text=True, errors="replace")
    print(result.stdout[-2000:], result.stderr[-2000:], sep="\n")
    if result.returncode != 0:
        sys.exit(f"Cooker failed for {package}: exit {result.returncode}")


def verify_blp(package, names, started_ns, stewpot):
    path = stewpot / f"{package}.blp"
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
    required += [ORIGINALS / "crests" / f"{name.lower()}.png" for name in CREST_NAMES]
    if growth_available:
        required.append(GROWTH_ORIGINALS / "notebooks_generated.png")
    required += [ORIGINALS / name for name in ABILITY_SOURCES]
    missing = [str(path) for path in required if not path.is_file()]
    if missing:
        sys.exit("Missing local originals:\n" + "\n".join(missing))
    if not all((COOKER_DIR / name).is_file() for name in ("Civ6AssetCooker_FinalRelease.exe", "Civ6.cfg")):
        sys.exit(f"Missing Civ6 SDK Asset Cooker: {COOKER_DIR}")
    if not growth_available:
        print(f"SKIP GrowthUI DDS: mod folder absent at {GROWTH_MOD} (set JAC_GROWTH_MOD to override)")

    shutil.rmtree(BUILD, ignore_errors=True)
    (BUILD / "Textures").mkdir(parents=True, exist_ok=True)
    (BUILD / "XLPs").mkdir(exist_ok=True)
    PREVIEWS.mkdir(parents=True, exist_ok=True)

    order = LEADER_ORDER
    leader_masters = [leader_icon_master(k) for k in order]
    civ_masters = [symbol_icon_master(name) for name in ("oda_mokko_generated.png", "takeda_bishi_generated.png")]
    ability_masters = [symbol_icon_master(name) for name in ABILITY_SOURCES]
    icons = [texture(f"JAC_Leaders{s}", atlas(leader_masters, s, 8)) for s in LEADER_ICON_SIZES]
    for s in CIV_ICON_SIZES:
        icons.append(texture(f"JAC_Civs{s}", atlas(civ_masters, s)))
    for s in ABILITY_ICON_SIZES:
        icons.append(texture(f"JAC_Abilities{s}", atlas(ability_masters, s)))

    # Data/JAC_Crests_Icons.xml replaces all 22 civilization icons with this atlas.
    crests = crest_masters()
    crest_contact_sheet(crests)
    for s in CIV_ICON_SIZES:
        icons.append(texture(f"JAC_Crests{s}", grid_atlas(crests, s)))

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
    STAGING.mkdir(exist_ok=True)
    for package, (cls, names) in packages.items():
        write_xlp(package, cls, names)
        started_ns = time.time_ns()
        cook(package, STAGING)
        verify_blp(package, names, started_ns, STAGING)

    STEWPOT.mkdir(parents=True, exist_ok=True)
    for package in packages:
        destination = STEWPOT / f"{package}.blp"
        (STAGING / destination.name).replace(destination)
        print(f"Installed {destination}")

    if growth_available:
        growth_icon()
    contact_sheet(growth_available)


if __name__ == "__main__":
    main()
