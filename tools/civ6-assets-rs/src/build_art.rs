use crate::*;
use anyhow::ensure;
pub use civ6_data_rs::asset_dependencies::art::{leaders, LEADER_ORDER};
use image::{imageops, Rgba, RgbaImage};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{SystemTime, UNIX_EPOCH},
};
pub const LEADER_ICON_SIZES: [u32; 8] = [32, 45, 48, 50, 55, 64, 80, 256];
pub const CIV_ICON_SIZES: [u32; 13] = [22, 30, 32, 36, 44, 45, 48, 50, 64, 80, 128, 200, 256];
pub const ABILITY_ICON_SIZES: [u32; 13] = CIV_ICON_SIZES;
pub const ABILITY_SOURCES: [&str; 4] = [
    "oda_clan_ability_generated.png",
    "takeda_clan_ability_generated_v2.png",
    "oda_nobuhide_ability_generated.png",
    "takeda_nobutora_ability_generated.png",
];
pub const PORTRAIT_SIZE: (u32, u32) = (570, 1024);
pub const BACKGROUND_SIZE: (u32, u32) = (1920, 1080);
pub const DIPLOMACY_BACKGROUND_SIZE: (u32, u32) = (1600, 1000);
pub const FACE_FILL: f64 = 0.60;
pub const FACE_TOP: f64 = 0.28;
pub const RING: f64 = 0.075;
pub const NOTEBOOKS_GLYPH: u32 = 16;
pub type IconAttributes = BTreeMap<String, String>;
pub type IconRecords = (
    BTreeMap<String, IconAttributes>,
    BTreeMap<String, Vec<IconAttributes>>,
);
pub fn icon_records(files: &[PathBuf], allow_duplicates: bool) -> Result<IconRecords> {
    let mut defs = BTreeMap::new();
    let mut atlases = BTreeMap::<String, Vec<IconAttributes>>::new();
    for file in files {
        let xml = xml(file)?;
        if !allow_duplicates {
            if let Some(table) = xml.get_child("IconDefinitions") {
                ensure!(
                    elements(table).all(|e| e.name == "Row"),
                    "{}: IconDefinitions edits; define icons once with Row",
                    file.display()
                );
            }
        }
        for row in rows(&xml, "IconDefinitions") {
            let name = attr(row, "Name");
            ensure!(
                allow_duplicates || !defs.contains_key(name),
                "duplicate icon: {name}"
            );
            defs.entry(name.to_owned()).or_insert_with(|| {
                row.attributes
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            });
        }
        for row in rows(&xml, "IconTextureAtlases") {
            atlases.entry(attr(row, "Name").into()).or_default().push(
                row.attributes
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            );
        }
    }
    Ok((defs, atlases))
}
pub fn faces() -> Value {
    serde_json::from_str(include_str!("../tests/fixtures/leader_icon_faces.json")).unwrap()
}
pub fn local_art_root() -> PathBuf {
    if let Some(p) = std::env::var_os("JAC_ART_LOCAL") {
        let p = PathBuf::from(p);
        return if p.is_absolute() { p } else { root().join(p) };
    }
    for p in [
        root().parent().unwrap().join("civ6-art-local"),
        root()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("civ6-art-local"),
    ] {
        if p.is_dir() {
            return p;
        }
    }
    root().parent().unwrap().join("civ6-art-local")
}
pub fn originals() -> PathBuf {
    local_art_root().join("JAC_PoC_CustomCivs/Originals")
}
pub fn cooker_dir() -> PathBuf {
    std::env::var_os("JAC_COOKER_DIR").map(PathBuf::from).unwrap_or_else(||"C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI SDK/AssetModTools/Cooker".into())
}
pub fn write_dds(path: &Path, image: &RgbaImage) -> Result<()> {
    let (w, h) = image.dimensions();
    let mut bytes = vec![0u8; 128];
    bytes[..4].copy_from_slice(b"DDS ");
    for (offset, value) in [
        (4, 124u32),
        (8, 0x100f),
        (12, h),
        (16, w),
        (20, w * 4),
        (28, 1),
        (76, 32),
        (80, 0x41),
        (88, 32),
        (92, 0x000000ff),
        (96, 0x0000ff00),
        (100, 0x00ff0000),
        (104, 0xff000000),
        (108, 0x1000),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes.extend(image.as_raw());
    write(path, bytes)
}
pub fn write_tex(build: &Path, name: &str, size: (u32, u32), class: &str) -> Result<()> {
    let tags = if class == "Leader_Fallback" {
        vec![class, "Leader", "Fallback"]
    } else {
        vec![class]
    };
    let mut root = xmltree::Element::new("AssetObjects..TextureInstance");
    let mut export = xmltree::Element::new("m_ExportSettings");
    for (k, v) in [
        ("ePixelformat", "PF_R8G8B8A8_UNORM"),
        ("eFilterType", "FT_LANCZOS6"),
        ("bUseMips", "false"),
        ("iNumManualMips", "0"),
        ("bCompleteMipChain", "false"),
        ("fValueClampMin", "0.000000"),
        ("fValueClampMax", "1.000000"),
        ("fSupportScale", "1.000000"),
        ("fGammaIn", "2.200000"),
        ("fGammaOut", "2.200000"),
        ("iSlabWidth", "0"),
        ("iSlabHeight", "0"),
        ("iColorKeyX", "64"),
        ("iColorKeyY", "64"),
        ("iColorKeyZ", "64"),
        ("eExportMode", "TEXTURE_2D"),
        ("bSampleFromTopLayer", "false"),
    ] {
        push(&mut export, text_node(k, v));
    }
    push(&mut root, export);
    let mut params = xmltree::Element::new("m_CookParams");
    push(&mut params, xmltree::Element::new("m_Values"));
    push(&mut root, params);
    push(&mut root, version(1, 0, 0, 0));
    for (k, v) in [
        ("m_Height", size.1.to_string()),
        ("m_Width", size.0.to_string()),
        ("m_Depth", "1".into()),
        ("m_NumMipMaps", "0".into()),
    ] {
        push(&mut root, text_node(k, &v));
    }
    push(
        &mut root,
        node("m_SourceFilePath", &[("text", format!("{name}.dds"))]),
    );
    push(
        &mut root,
        node("m_SourceObjectName", &[("text", String::new())]),
    );
    for k in ["m_ImportedTime", "m_ExportedTime"] {
        push(&mut root, text_node(k, "0"));
    }
    push(&mut root, node("m_ClassName", &[("text", class.into())]));
    let mut files = xmltree::Element::new("m_DataFiles");
    let mut entry = xmltree::Element::new("Element");
    push(&mut entry, node("m_ID", &[("text", "DDS".into())]));
    push(
        &mut entry,
        node("m_RelativePath", &[("text", format!("{name}.dds"))]),
    );
    push(&mut files, entry);
    push(&mut root, files);
    push(&mut root, node("m_Name", &[("text", name.into())]));
    push(&mut root, node("m_Description", &[("text", String::new())]));
    let mut ts = xmltree::Element::new("m_Tags");
    for t in tags {
        push(&mut ts, node("Element", &[("text", t.into())]));
    }
    push(&mut root, ts);
    push(&mut root, xmltree::Element::new("m_Groups"));
    xml_write(build.join("Textures").join(format!("{name}.tex")), &root)
}
fn text_node(name: &str, text: &str) -> xmltree::Element {
    let mut e = xmltree::Element::new(name);
    e.children.push(xmltree::XMLNode::Text(text.into()));
    e
}
fn version(a: i32, b: i32, c: i32, d: i32) -> xmltree::Element {
    let mut e = xmltree::Element::new("m_Version");
    for (k, v) in [("major", a), ("minor", b), ("build", c), ("revision", d)] {
        push(&mut e, text_node(k, &v.to_string()));
    }
    e
}
pub fn write_xlp(build: &Path, package: &str, class: &str, names: &[String]) -> Result<()> {
    let mut root = xmltree::Element::new("AssetObjects..XLP");
    push(&mut root, version(4, 0, 253, 293));
    push(&mut root, node("m_ClassName", &[("text", class.into())]));
    push(
        &mut root,
        node("m_PackageName", &[("text", package.into())]),
    );
    let mut entries = xmltree::Element::new("m_Entries");
    for name in names {
        let mut e = xmltree::Element::new("Element");
        push(&mut e, node("m_EntryID", &[("text", name.clone())]));
        push(&mut e, node("m_ObjectName", &[("text", name.clone())]));
        push(&mut entries, e);
    }
    push(&mut root, entries);
    let mut platforms = xmltree::Element::new("m_AllowedPlatforms");
    push(&mut platforms, text_node("Element", "WINDOWS"));
    push(&mut root, platforms);
    xml_write(build.join("XLPs").join(format!("{package}.xlp")), &root)
}
pub fn texture(build: &Path, name: &str, im: &RgbaImage, class: &str) -> Result<String> {
    write_dds(&build.join("Textures").join(format!("{name}.dds")), im)?;
    write_tex(build, name, im.dimensions(), class)?;
    fs::create_dir_all(build.join("previews"))?;
    im.save(build.join("previews").join(format!("{name}.png")))?;
    Ok(name.into())
}
pub fn resize(im: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    if im.dimensions() == (w, h) {
        return im.clone();
    }
    if im.width() == 0 || im.height() == 0 || w == 0 || h == 0 {
        return RgbaImage::new(w, h);
    }
    use fast_image_resize::{
        images::{Image, ImageRef},
        FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer,
    };
    let src =
        ImageRef::new(im.width(), im.height(), im.as_raw(), PixelType::U8x4).expect("RGBA buffer");
    let mut dst = Image::new(w, h, PixelType::U8x4);
    Resizer::new()
        .resize(
            &src,
            &mut dst,
            &ResizeOptions::new()
                .resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3))
                .use_alpha(true),
        )
        .expect("valid resize dimensions");
    RgbaImage::from_raw(w, h, dst.into_vec()).expect("resized RGBA buffer")
}
pub fn alpha_bounds(image: &RgbaImage, threshold: u8) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (image.width(), image.height(), 0, 0);
    for (x, y, p) in image.enumerate_pixels() {
        if p[3] >= threshold {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
}
pub fn key_magenta(path: &Path) -> Result<RgbaImage> {
    let mut image = image::open(path)?.to_rgba8();
    let (w, h) = image.dimensions();
    for p in image.pixels_mut() {
        let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
        let distance = (255 - r).max(g).max(255 - b);
        let alpha = round((distance - 18) as f64 * 255. / 75., 0).clamp(0., 255.) as u8;
        let magenta = r.min(b).saturating_sub(g).max(0);
        let hue = round((100 - magenta) as f64 * 255. / 65., 0).clamp(0., 255.) as u8;
        p[3] = alpha.min(hue);
    }
    let old = image.clone();
    for y in 0..h {
        for x in 0..w {
            let mut min = 255;
            for dy in -2..=2 {
                for dx in -2..=2 {
                    min = min.min(
                        old.get_pixel(
                            (x as i32 + dx).clamp(0, w as i32 - 1) as u32,
                            (y as i32 + dy).clamp(0, h as i32 - 1) as u32,
                        )[3],
                    );
                }
            }
            image.get_pixel_mut(x, y)[3] = min;
        }
    }
    Ok(image)
}
fn thumbnail(im: &RgbaImage, bounds: (u32, u32)) -> RgbaImage {
    let ratio = (bounds.0 as f64 / im.width() as f64)
        .min(bounds.1 as f64 / im.height() as f64)
        .min(1.);
    resize(
        im,
        round(im.width() as f64 * ratio, 0).max(1.) as u32,
        round(im.height() as f64 * ratio, 0).max(1.) as u32,
    )
}
pub fn foreground(originals: &Path, key: &str, max_height: u32) -> Result<RgbaImage> {
    let keyed = key_magenta(
        &originals
            .join(key.to_lowercase())
            .join(generate_portraits::filename(key, "figure")),
    )?;
    let (x0, y0, x1, y1) =
        alpha_bounds(&keyed, 1).with_context(|| format!("Empty keyed figure: {key}"))?;
    let cut = thumbnail(
        &imageops::crop_imm(&keyed, x0, y0, x1 - x0, y1 - y0).to_image(),
        (550, max_height),
    );
    let mut out = RgbaImage::new(PORTRAIT_SIZE.0, PORTRAIT_SIZE.1);
    imageops::overlay(
        &mut out,
        &cut,
        ((PORTRAIT_SIZE.0 - cut.width()) / 2) as i64,
        (PORTRAIT_SIZE.1 - cut.height()) as i64,
    );
    Ok(out)
}
pub fn background(originals: &Path, key: &str, size: (u32, u32)) -> Result<RgbaImage> {
    let src = image::open(
        originals
            .join(key.to_lowercase())
            .join(generate_portraits::filename(key, "background")),
    )?
    .to_rgba8();
    let ratio = (size.0 as f64 / src.width() as f64).max(size.1 as f64 / src.height() as f64);
    let fitted = resize(
        &src,
        (src.width() as f64 * ratio).ceil() as u32,
        (src.height() as f64 * ratio).ceil() as u32,
    );
    Ok(imageops::crop_imm(
        &fitted,
        (fitted.width() - size.0) / 2,
        (fitted.height() - size.1) / 2,
        size.0,
        size.1,
    )
    .to_image())
}
pub fn house_colors(game: &Path, mod_dir: &Path) -> Result<BTreeMap<String, [u8; 3]>> {
    let standard = xml(game.join("Base/Assets/UI/Colors/PlayerStandardColors.xml"))?;
    let mut colors = BTreeMap::new();
    for row in rows(&standard, "Colors") {
        let values = text_of(row, "Color")
            .split(',')
            .take(3)
            .map(str::parse::<u8>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(values.len() == 3, "standard color invalid");
        colors.insert(text_of(row, "Type"), [values[0], values[1], values[2]]);
    }
    let mut out = BTreeMap::new();
    for file in ["JAC_CustomCivs_Colors.xml", "JAC_Japan1534_Colors.xml"] {
        for row in rows(&xml(mod_dir.join("Data").join(file))?, "PlayerColors") {
            let color = text_of(row, "PrimaryColor");
            out.insert(
                text_of(row, "Type")
                    .trim_start_matches("LEADER_JAC_")
                    .to_owned(),
                *colors.get(&color).context("house standard color missing")?,
            );
        }
    }
    Ok(out)
}
pub fn leader_icon_master(
    originals: &Path,
    key: &str,
    colors: &BTreeMap<String, [u8; 3]>,
) -> Result<RgbaImage> {
    let src = image::open(
        originals
            .join(key.to_lowercase())
            .join(generate_portraits::filename(key, "portrait")),
    )?
    .to_rgba8();
    let face = faces();
    let box_ = arr(&face[key]);
    ensure!(box_.len() == 4, "face coordinates missing");
    let side = (f(&box_[3]) - f(&box_[1])) * src.height() as f64 / FACE_FILL;
    let cx = (f(&box_[0]) + f(&box_[2])) / 2. * src.width() as f64;
    let top = f(&box_[1]) * src.height() as f64 - FACE_TOP * side;
    let (x0, y0, x1, y1) = (
        round(cx - side / 2., 0) as i32,
        round(top, 0) as i32,
        round(cx + side / 2., 0) as i32,
        round(top + side, 0) as i32,
    );
    ensure!(
        x0 >= 0 && y0 >= 0 && x1 <= src.width() as i32 && y1 <= src.height() as i32,
        "Face crop leaves portrait: {key}"
    );
    let big = 2048;
    let ring = round(big as f64 * RING, 0) as u32;
    let line = 20;
    let face = resize(
        &imageops::crop_imm(
            &src,
            x0 as u32,
            y0 as u32,
            (x1 - x0) as u32,
            (y1 - y0) as u32,
        )
        .to_image(),
        big - 2 * ring,
        big - 2 * ring,
    );
    let color = colors.get(key).context("house color missing")?;
    let mut out = RgbaImage::new(big, big);
    for y in 0..big {
        for x in 0..big {
            let d = (x as f64 - (big - 1) as f64 / 2.).hypot(y as f64 - (big - 1) as f64 / 2.);
            if d < big as f64 / 2. {
                let p = if d > big as f64 / 2. - line as f64 {
                    [20, 16, 12, 255]
                } else {
                    [color[0], color[1], color[2], 255]
                };
                out.put_pixel(x, y, Rgba(p));
            }
            if x >= ring && y >= ring && x < big - ring && y < big - ring {
                let inner = (big - 2 * ring) as f64 / 2.;
                if d < inner {
                    out.put_pixel(x, y, *face.get_pixel(x - ring, y - ring));
                }
                if d >= inner && d <= inner + line as f64 {
                    out.put_pixel(x, y, Rgba([20, 16, 12, 255]));
                }
            }
        }
    }
    Ok(resize(&out, 512, 512))
}
pub fn symbol_icon_master(originals: &Path, filename: &str) -> Result<RgbaImage> {
    let rgb = image::open(originals.join(filename))?.to_rgb8();
    let mut out = RgbaImage::new(rgb.width(), rgb.height());
    for (x, y, p) in rgb.enumerate_pixels() {
        let v = round(
            (299 * p[0] as i32 + 587 * p[1] as i32 + 114 * p[2] as i32) as f64 / 1000.,
            0,
        );
        out.put_pixel(
            x,
            y,
            Rgba([
                255,
                255,
                255,
                round((220. - v) * 255. / 170., 0).clamp(0., 255.) as u8,
            ]),
        );
    }
    let (x0, y0, x1, y1) =
        alpha_bounds(&out, 101).with_context(|| format!("Empty generated mon: {filename}"))?;
    let cut = thumbnail(
        &imageops::crop_imm(&out, x0, y0, x1 - x0, y1 - y0).to_image(),
        (460, 460),
    );
    let mut white = RgbaImage::from_pixel(512, 512, Rgba([255, 255, 255, 0]));
    imageops::overlay(
        &mut white,
        &cut,
        ((512 - cut.width()) / 2) as i64,
        ((512 - cut.height()) / 2) as i64,
    );
    Ok(white)
}
pub fn atlas(masters: &[RgbaImage], size: u32, per_row: Option<usize>) -> Result<RgbaImage> {
    ensure!(size > 0 && !masters.is_empty(), "empty atlas");
    let columns = per_row.map(u32::try_from).transpose()?;
    // The shared helper owns layout, inventory checks and composition. Resize
    // with alpha premultiplication before passing the final cells to it.
    let cells = masters
        .iter()
        .map(|m| resize(m, size, size))
        .collect::<Vec<_>>();
    civ6_data_rs::asset_dependencies::art::atlas(&cells, size, columns)
}
pub fn grid_atlas(masters: &[RgbaImage], size: u32, columns: usize) -> Result<RgbaImage> {
    atlas(masters, size, Some(columns))
}
pub fn crest_masters(originals: &Path) -> Result<Vec<RgbaImage>> {
    draw_crests::NAMES
        .iter()
        .map(|name| {
            let im = image::open(
                originals
                    .join("crests")
                    .join(format!("{}.png", name.to_lowercase())),
            )?
            .to_rgba8();
            ensure!(
                im.dimensions() == (512, 512) && alpha_bounds(&im, 1).is_some(),
                "Invalid crest image: {name}"
            );
            Ok(im)
        })
        .collect()
}
pub fn label(im: &mut RgbaImage, x: u32, y: u32, text: &str) {
    use font8x8::UnicodeFonts;
    for (i, c) in text.chars().enumerate() {
        if let Some(bits) = font8x8::BASIC_FONTS.get(c) {
            for (dy, row) in bits.iter().enumerate() {
                for dx in 0..8 {
                    let px = x + i as u32 * 8 + dx;
                    let py = y + dy as u32;
                    if row & (1 << dx) != 0 && px < im.width() && py < im.height() {
                        im.put_pixel(px, py, Rgba([255; 4]));
                    }
                }
            }
        }
    }
}
pub fn crest_contact_sheet(build: &Path, masters: &[RgbaImage]) -> Result<()> {
    ensure!(masters.len() == draw_crests::NAMES.len(), "crest inventory");
    let mut sheet = RgbaImage::from_pixel(1080, 780, Rgba([69, 72, 77, 255]));
    for (i, (name, master)) in draw_crests::NAMES.iter().zip(masters).enumerate() {
        let (x, y) = ((i % 6) as u32 * 180, (i / 6) as u32 * 195);
        imageops::overlay(
            &mut sheet,
            &resize(master, 104, 104),
            (x + 22) as i64,
            (y + 12) as i64,
        );
        imageops::overlay(
            &mut sheet,
            &resize(master, 22, 22),
            (x + 135) as i64,
            (y + 60) as i64,
        );
        label(&mut sheet, x + 8, y + 143, name);
    }
    fs::create_dir_all(build.join("previews"))?;
    sheet.save(build.join("previews/house-crests.png"))?;
    Ok(())
}
pub fn growth_icon(originals: &Path, destination: &Path, build: &Path) -> Result<RgbaImage> {
    let keyed = key_magenta(&originals.join("notebooks_generated.png"))?;
    let (x0, y0, x1, y1) = alpha_bounds(&keyed, 1).context("Empty notebooks icon")?;
    let cut = thumbnail(
        &imageops::crop_imm(&keyed, x0, y0, x1 - x0, y1 - y0).to_image(),
        (NOTEBOOKS_GLYPH, NOTEBOOKS_GLYPH),
    );
    let mut out = RgbaImage::new(26, 26);
    imageops::overlay(
        &mut out,
        &cut,
        ((26 - cut.width()) / 2) as i64,
        ((26 - cut.height()) / 2) as i64,
    );
    write_dds(destination, &out)?;
    fs::create_dir_all(build.join("previews"))?;
    out.save(build.join("previews/JAC_Notebooks_Hook.png"))?;
    println!(
        "OK {} {} bytes",
        destination.display(),
        fs::metadata(destination)?.len()
    );
    Ok(out)
}
pub fn contact_sheet(
    build: &Path,
    originals: &Path,
    colors: &BTreeMap<String, [u8; 3]>,
    include_growth: bool,
) -> Result<()> {
    let mut names = LEADER_ICON_SIZES
        .into_iter()
        .map(|s| format!("JAC_Leaders{s}"))
        .chain(CIV_ICON_SIZES.into_iter().map(|s| format!("JAC_Crests{s}")))
        .chain(
            ABILITY_ICON_SIZES
                .into_iter()
                .map(|s| format!("JAC_Abilities{s}")),
        )
        .collect::<Vec<_>>();
    for key in LEADER_ORDER {
        names.extend([
            format!("LEADER_JAC_{key}_NEUTRAL"),
            format!("LEADER_JAC_{key}_BACKGROUND"),
            format!("LEADER_JAC_{key}_DIPLOMACY_BACKGROUND"),
            format!("FALLBACK_NEUTRAL_JAC_{key}"),
        ]);
    }
    if include_growth {
        names.push("JAC_Notebooks_Hook".into());
    }
    let mut sheet = RgbaImage::from_pixel(
        1680,
        names.len().div_ceil(4) as u32 * 320,
        Rgba([34, 39, 48, 255]),
    );
    for (i, name) in names.iter().enumerate() {
        let (x, y) = ((i % 4) as u32 * 420, (i / 4) as u32 * 320);
        for py in y + 8..y + 312 {
            for px in x + 8..x + 412 {
                sheet.put_pixel(px, py, Rgba([65, 72, 82, 255]));
            }
        }
        label(&mut sheet, x + 18, y + 15, name);
        let im = thumbnail(
            &image::open(build.join("previews").join(format!("{name}.png")))?.to_rgba8(),
            (384, 265),
        );
        let mut pane = RgbaImage::from_pixel(384, 265, Rgba([115, 123, 133, 255]));
        imageops::overlay(
            &mut pane,
            &im,
            ((384 - im.width()) / 2) as i64,
            ((265 - im.height()) / 2) as i64,
        );
        imageops::overlay(&mut sheet, &pane, (x + 18) as i64, (y + 43) as i64);
    }
    sheet.save(build.join("previews/contact-sheet.png"))?;
    let mut face_sheet = RgbaImage::from_pixel(
        1040,
        LEADER_ORDER.len().div_ceil(4) as u32 * 290,
        Rgba([34, 39, 48, 255]),
    );
    for (i, key) in LEADER_ORDER.into_iter().enumerate() {
        let (x, y) = ((i % 4) as u32 * 260, (i / 4) as u32 * 290);
        imageops::overlay(
            &mut face_sheet,
            &resize(&leader_icon_master(originals, key, colors)?, 220, 220),
            (x + 20) as i64,
            (y + 18) as i64,
        );
        label(&mut face_sheet, x + 12, y + 248, key);
    }
    face_sheet.save(build.join("previews/portrait-sheet.png"))?;
    Ok(())
}
pub fn cook(cooker: &Path, build: &Path, package: &str, stewpot: &Path) -> Result<()> {
    let cooker = std::path::absolute(cooker)?;
    let build = std::path::absolute(build)?;
    let stewpot = std::path::absolute(stewpot)?;
    let mut command = Command::new(cooker.join("Civ6AssetCooker_FinalRelease.exe"));
    command
        .args([
            "--no_mt",
            "--mode",
            "XLP",
            "--platform",
            "Windows",
            "--pantry",
        ])
        .arg(&build)
        .arg("--stewpot")
        .arg(&stewpot)
        .arg("--config")
        .arg(cooker.join("Civ6.cfg"))
        .arg(build.join("XLPs").join(format!("{package}.xlp")))
        .current_dir(&build);
    hidden(&mut command);
    let result = command.output()?;
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    println!(
        "{}\n{}",
        stdout
            .chars()
            .rev()
            .take(2000)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>(),
        stderr
            .chars()
            .rev()
            .take(2000)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>()
    );
    ensure!(
        result.status.success(),
        "Cooker failed for {package}: exit {}",
        result.status
    );
    Ok(())
}
pub fn package_textures(path: &Path, names: &BTreeSet<String>) -> Result<Vec<String>> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() == 0 {
        return Ok(names.iter().cloned().collect());
    }
    let mapped = unsafe { memmap2::MmapOptions::new().map(&file)? };
    Ok(names
        .iter()
        .filter(|name| {
            !mapped
                .windows(name.len().max(1))
                .any(|w| w == name.as_bytes())
        })
        .cloned()
        .collect())
}
pub fn verify_blp(
    package: &str,
    names: &[String],
    started_ns: u128,
    stewpot: &Path,
) -> Result<String> {
    let path = stewpot.join(format!("{package}.blp"));
    let meta =
        fs::metadata(&path).with_context(|| format!("Cooker did not create {}", path.display()))?;
    let modified = meta.modified()?.duration_since(UNIX_EPOCH)?.as_nanos();
    ensure!(
        meta.len() >= 1024 && modified >= started_ns,
        "Cooker output is empty or stale: {} ({} bytes, mtime={modified})",
        path.display(),
        meta.len()
    );
    let missing = package_textures(&path, &names.iter().cloned().collect())?;
    ensure!(missing.is_empty(), "{} lacks {missing:?}", path.display());
    let digest = sha256(&fs::read(&path)?);
    println!(
        "OK {} {} bytes, {} textures, mtime_ns={modified}, sha256={digest}",
        path.file_name().unwrap().to_string_lossy(),
        meta.len(),
        names.len()
    );
    Ok(digest)
}
#[derive(Clone, Debug)]
pub struct BuildOptions {
    pub originals: PathBuf,
    pub growth_originals: PathBuf,
    pub mod_dir: PathBuf,
    pub growth_mod: PathBuf,
    pub build: PathBuf,
    pub cooker: PathBuf,
    pub game: PathBuf,
}
impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            originals: originals(),
            growth_originals: local_art_root().join("JAC_PoC_GrowthUI/Originals"),
            mod_dir: root().join("poc/JAC_PoC_CustomCivs"),
            growth_mod: std::env::var_os("JAC_GROWTH_MOD")
                .map(PathBuf::from)
                .unwrap_or_else(|| root().join("poc/JAC_PoC_GrowthUI")),
            build: root().join("tools/jac_art/build"),
            cooker: cooker_dir(),
            game: generate::game_dir(),
        }
    }
}
pub fn build(options: &BuildOptions) -> Result<()> {
    let o = options;
    let growth = o.growth_mod.join("JAC_PoC_GrowthUI.modinfo").is_file();
    let mut required = LEADER_ORDER
        .into_iter()
        .flat_map(|key| {
            ["portrait", "figure", "background"]
                .into_iter()
                .map(move |kind| {
                    o.originals
                        .join(key.to_lowercase())
                        .join(generate_portraits::filename(key, kind))
                })
        })
        .collect::<Vec<_>>();
    required.extend(draw_crests::NAMES.into_iter().map(|n| {
        o.originals
            .join("crests")
            .join(format!("{}.png", n.to_lowercase()))
    }));
    required.extend(ABILITY_SOURCES.into_iter().map(|s| o.originals.join(s)));
    if growth {
        required.push(o.growth_originals.join("notebooks_generated.png"));
    }
    let missing: Vec<_> = required.iter().filter(|p| !p.is_file()).collect();
    ensure!(missing.is_empty(), "Missing originals: {missing:?}");
    ensure!(
        ["Civ6AssetCooker_FinalRelease.exe", "Civ6.cfg"]
            .iter()
            .all(|n| o.cooker.join(n).is_file()),
        "Missing Civ6 SDK Asset Cooker: {}",
        o.cooker.display()
    );
    let colors = house_colors(&o.game, &o.mod_dir)?;
    fs::create_dir_all(o.build.join("Textures"))?;
    fs::create_dir_all(o.build.join("XLPs"))?;
    fs::create_dir_all(o.build.join("previews"))?;
    let masters = LEADER_ORDER
        .into_iter()
        .map(|key| leader_icon_master(&o.originals, key, &colors))
        .collect::<Result<Vec<_>>>()?;
    let abilities = ABILITY_SOURCES
        .into_iter()
        .map(|n| symbol_icon_master(&o.originals, n))
        .collect::<Result<Vec<_>>>()?;
    let mut icons = vec![];
    for size in LEADER_ICON_SIZES {
        icons.push(texture(
            &o.build,
            &format!("JAC_Leaders{size}"),
            &atlas(&masters, size, Some(8))?,
            "UserInterface",
        )?);
    }
    for size in ABILITY_ICON_SIZES {
        icons.push(texture(
            &o.build,
            &format!("JAC_Abilities{size}"),
            &atlas(&abilities, size, None)?,
            "UserInterface",
        )?);
    }
    let crests = crest_masters(&o.originals)?;
    crest_contact_sheet(&o.build, &crests)?;
    for size in CIV_ICON_SIZES {
        icons.push(texture(
            &o.build,
            &format!("JAC_Crests{size}"),
            &grid_atlas(&crests, size, 8)?,
            "UserInterface",
        )?);
    }
    let (mut leaders, mut fallbacks) = (vec![], vec![]);
    for key in LEADER_ORDER {
        leaders.push(texture(
            &o.build,
            &format!("LEADER_JAC_{key}_NEUTRAL"),
            &foreground(&o.originals, key, 860)?,
            "UserInterface",
        )?);
        leaders.push(texture(
            &o.build,
            &format!("LEADER_JAC_{key}_BACKGROUND"),
            &background(&o.originals, key, BACKGROUND_SIZE)?,
            "UserInterface",
        )?);
        leaders.push(texture(
            &o.build,
            &format!("LEADER_JAC_{key}_DIPLOMACY_BACKGROUND"),
            &background(&o.originals, key, DIPLOMACY_BACKGROUND_SIZE)?,
            "UserInterface",
        )?);
        fallbacks.push(texture(
            &o.build,
            &format!("FALLBACK_NEUTRAL_JAC_{key}"),
            &foreground(&o.originals, key, 1010)?,
            "Leader_Fallback",
        )?);
    }
    let packages = [
        ("JAC_CustomCivs_Icons", "UITexture", icons),
        ("JAC_CustomCivs_Leaders", "UITexture", leaders),
        ("JAC_CustomCivs_Fallback", "LeaderFallback", fallbacks),
    ];
    let staging = o.build.join("cooked");
    fs::create_dir_all(&staging)?;
    for (package, class, names) in &packages {
        write_xlp(&o.build, package, class, names)?;
        let prior = staging.join(format!("{package}.blp"));
        if prior.exists() {
            fs::remove_file(prior)?;
        }
        let started = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        cook(&o.cooker, &o.build, package, &staging)?;
        verify_blp(package, names, started, &staging)?;
    }
    let stewpot = o.mod_dir.join("Platforms/Windows/BLPs");
    fs::create_dir_all(&stewpot)?;
    for (package, _, _) in packages {
        let src = staging.join(format!("{package}.blp"));
        let dest = stewpot.join(format!("{package}.blp"));
        fs::rename(src, &dest)?;
        println!("Installed {}", dest.display());
    }
    if growth {
        growth_icon(
            &o.growth_originals,
            &o.growth_mod.join("UI/Textures/JAC_Notebooks_Hook.dds"),
            &o.build,
        )?;
    } else {
        println!(
            "GrowthUI DDS omitted: mod folder absent at {}",
            o.growth_mod.display()
        );
    }
    contact_sheet(&o.build, &o.originals, &colors, growth)
}
