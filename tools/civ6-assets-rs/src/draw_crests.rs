use crate::*;
use anyhow::{bail, ensure};
use image::{Rgba, RgbaImage};
use serde_json::json;
use xmltree::Element;
pub const SIZE: u32 = 512;
pub const SCALE: u32 = 4;
pub const NAMES: [&str; 22] = [
    "ODA",
    "TAKEDA",
    "DATE",
    "ASHINA",
    "HOJO",
    "YAMANOUCHI_UESUGI",
    "NAGAO",
    "IMAGAWA",
    "MATSUDAIRA",
    "ASAKURA",
    "ASHIKAGA",
    "MIYOSHI",
    "ROKKAKU",
    "HONGANJI",
    "OUCHI",
    "AMAGO",
    "MORI",
    "CHOSOKABE",
    "OTOMO",
    "SHIMAZU",
    "RYUKYU",
    "AINU",
];
pub const MOTIFS: [&str; 22] = [
    "織田木瓜",
    "武田菱",
    "竪三引両",
    "丸に三引両",
    "三つ鱗",
    "上杉笹",
    "九曜",
    "今川赤鳥",
    "丸に三つ葉葵",
    "三つ盛木瓜",
    "足利二つ引",
    "三階菱に五つ釘抜",
    "隅立て四つ目",
    "下り藤（1534年への適用は暫定）",
    "大内菱",
    "平四つ目結",
    "一文字三星",
    "七つ酢漿草",
    "杏葉紋",
    "島津十文字",
    "左三巴",
    "サケ（暫定・創作）",
];
pub type Point = (f64, f64);
pub struct Crest {
    pub root: Element,
}
impl Default for Crest {
    fn default() -> Self {
        Self::new()
    }
}
impl Crest {
    pub fn new() -> Self {
        Self {
            root: node(
                "svg",
                &[
                    ("xmlns", "http://www.w3.org/2000/svg".into()),
                    ("width", SIZE.to_string()),
                    ("height", SIZE.to_string()),
                    ("viewBox", "0 0 512 512".into()),
                ],
            ),
        }
    }
    pub fn polygon(&mut self, points: &[Point], fill: &str, stroke: Option<f64>) {
        let mut attrs = vec![
            (
                "points",
                points
                    .iter()
                    .map(|(x, y)| format!("{x:.2},{y:.2}"))
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            ("fill", fill.into()),
        ];
        if let Some(width) = stroke {
            attrs.extend([
                ("stroke", "white".into()),
                ("stroke-width", width.to_string()),
            ]);
        }
        push(&mut self.root, node("polygon", &attrs));
    }
    pub fn circle(&mut self, x: f64, y: f64, r: f64, fill: &str, stroke: Option<f64>) {
        let mut attrs = vec![
            ("cx", format!("{x:.2}")),
            ("cy", format!("{y:.2}")),
            ("r", format!("{r:.2}")),
            ("fill", fill.into()),
        ];
        if let Some(width) = stroke {
            attrs.extend([
                ("stroke", "white".into()),
                ("stroke-width", width.to_string()),
            ]);
        }
        push(&mut self.root, node("circle", &attrs));
    }
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, fill: &str) {
        push(
            &mut self.root,
            node(
                "rect",
                &[
                    ("x", x.to_string()),
                    ("y", y.to_string()),
                    ("width", w.to_string()),
                    ("height", h.to_string()),
                    ("fill", fill.into()),
                ],
            ),
        );
    }
    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, width: f64) {
        push(
            &mut self.root,
            node(
                "line",
                &[
                    ("x1", x1.to_string()),
                    ("y1", y1.to_string()),
                    ("x2", x2.to_string()),
                    ("y2", y2.to_string()),
                    ("stroke", "white".into()),
                    ("stroke-width", width.to_string()),
                    ("stroke-linecap", "round".into()),
                ],
            ),
        );
    }
    pub fn ring(&mut self, r: f64, width: f64) {
        self.circle(256., 256., r, "none", Some(width));
    }
    pub fn diamond(&mut self, x: f64, y: f64, rx: f64, ry: f64) {
        self.polygon(
            &[(x, y - ry), (x + rx, y), (x, y + ry), (x - rx, y)],
            "white",
            None,
        );
    }
    pub fn leaf(&mut self, x: f64, y: f64, len: f64, half: f64, angle: f64) {
        let a = angle.to_radians();
        let mut points = vec![];
        for side in [1., -1.] {
            let indices: Vec<_> = if side == 1. {
                (0..25).collect()
            } else {
                (0..25).rev().collect()
            };
            for i in indices {
                let t = i as f64 / 24.;
                let lx = side * half * (std::f64::consts::PI * t).sin().powf(0.8);
                let ly = -len * t;
                points.push((
                    x + lx * a.cos() - ly * a.sin(),
                    y + lx * a.sin() + ly * a.cos(),
                ));
            }
        }
        self.polygon(&points, "white", None);
    }
}
pub fn rotate(points: &[Point], angle: f64, cx: f64, cy: f64) -> Vec<Point> {
    let a = angle.to_radians();
    points
        .iter()
        .map(|&(x, y)| {
            (
                cx + (x - cx) * a.cos() - (y - cy) * a.sin(),
                cy + (x - cx) * a.sin() + (y - cy) * a.cos(),
            )
        })
        .collect()
}
pub fn cubic(start: Point, c1: Point, c2: Point, end: Point, steps: usize) -> Vec<Point> {
    (0..=steps)
        .map(|i| {
            let t = i as f64 / steps as f64;
            (
                (1. - t).powi(3) * start.0
                    + 3. * (1. - t).powi(2) * t * c1.0
                    + 3. * (1. - t) * t * t * c2.0
                    + t.powi(3) * end.0,
                (1. - t).powi(3) * start.1
                    + 3. * (1. - t).powi(2) * t * c1.1
                    + 3. * (1. - t) * t * t * c2.1
                    + t.powi(3) * end.1,
            )
        })
        .collect()
}
pub fn flower(c: &mut Crest, x: f64, y: f64, r: f64, petals: usize) {
    for i in 0..petals {
        let a = i as f64 * 360. / petals as f64;
        let rad = a.to_radians();
        c.leaf(
            x + 0.17 * r * rad.sin(),
            y - 0.17 * r * rad.cos(),
            r,
            r * 0.35,
            a,
        );
    }
    c.circle(x, y, r * 0.19, "white", None);
}
pub fn draw(name: &str) -> Result<Crest> {
    let mut c = Crest::new();
    match name {
        "ODA" => {
            c.ring(190., 18.);
            flower(&mut c, 256., 256., 125., 5);
            for a in 0..5 {
                let r = (a as f64 * 72.).to_radians();
                c.circle(
                    256. + 119. * r.sin(),
                    256. - 119. * r.cos(),
                    11.,
                    "white",
                    None,
                );
            }
        }
        "TAKEDA" => {
            for x in [181., 331.] {
                for y in [181., 331.] {
                    c.diamond(x, y, 75., 75.);
                }
            }
        }
        "DATE" => {
            c.ring(190., 18.);
            for x in [181., 256., 331.] {
                c.rect(x - 22., 123., 44., 266., "white");
            }
        }
        "ASHINA" => {
            c.ring(190., 18.);
            for y in [169., 256., 343.] {
                c.rect(123., y - 19., 266., 38., "white");
            }
        }
        "HOJO" => {
            for (x, y) in [(256., 110.), (145., 298.), (367., 298.)] {
                c.polygon(
                    &[(x, y), (x - 100., y + 173.), (x + 100., y + 173.)],
                    "white",
                    None,
                );
            }
        }
        "YAMANOUCHI_UESUGI" => {
            c.ring(190., 18.);
            for side in [-1., 1.] {
                c.line(256. + side * 69., 370., 256. + side * 129., 133., 15.);
                for j in 0..4 {
                    let y = 178. + j as f64 * 53.;
                    c.leaf(
                        256. + side * (128. - j as f64 * 13.),
                        y,
                        77.,
                        17.,
                        side * if j % 2 == 1 { 55. } else { -55. },
                    );
                }
                let x = 256. + side * 54.;
                c.circle(x, 271., 17., "white", None);
                c.leaf(x, 320., 54., 24., side * 65.);
                c.polygon(
                    &[
                        (x, 286.),
                        (x + side * 21., 319.),
                        (x, 329.),
                        (x - side * 23., 309.),
                    ],
                    "white",
                    None,
                );
            }
        }
        "NAGAO" => {
            c.circle(256., 256., 47., "none", Some(18.));
            c.circle(256., 256., 15., "white", None);
            for i in 0..8 {
                let a = (i as f64 * 45.).to_radians();
                let (x, y) = (256. + 126. * a.sin(), 256. - 126. * a.cos());
                c.circle(x, y, 35., "none", Some(13.));
                c.circle(x, y, 10., "white", None);
            }
        }
        "IMAGAWA" => {
            c.ring(190., 18.);
            c.circle(256., 149., 31., "white", None);
            c.rect(128., 167., 256., 100., "white");
            c.polygon(
                &[(128., 267.), (154., 299.), (358., 299.), (384., 267.)],
                "white",
                None,
            );
            for x in [164., 210., 256., 302., 348.] {
                c.polygon(
                    &[
                        (x - 13., 278.),
                        (x + 13., 278.),
                        (x + 11., 381.),
                        (x, 398.),
                        (x - 11., 381.),
                    ],
                    "white",
                    None,
                );
            }
        }
        "MATSUDAIRA" => {
            c.ring(190., 18.);
            for a in [0f64, 120., 240.] {
                let r = a.to_radians();
                c.leaf(256. + 20. * r.sin(), 256. - 20. * r.cos(), 135., 49., a);
            }
            c.circle(256., 256., 26., "white", None);
        }
        "ASAKURA" => {
            for (x, y) in [(256., 160.), (164., 323.), (348., 323.)] {
                c.circle(x, y, 82., "none", Some(13.));
                flower(&mut c, x, y, 54., 4);
            }
        }
        "ASHIKAGA" => {
            for y in [201., 311.] {
                c.rect(93., y - 23., 326., 46., "white");
            }
        }
        "MIYOSHI" => {
            for (x, y, w) in [(256., 143., 58.), (256., 231., 89.), (256., 353., 125.)] {
                c.diamond(x, y, w, 37.);
            }
            for (x, y) in [
                (170., 196.),
                (342., 196.),
                (127., 292.),
                (256., 292.),
                (385., 292.),
            ] {
                c.diamond(x, y, 20., 20.);
                c.polygon(
                    &[(x, y - 8.), (x + 8., y), (x, y + 8.), (x - 8., y)],
                    "black",
                    None,
                );
            }
        }
        "ROKKAKU" => {
            for x in [174., 338.] {
                for y in [174., 338.] {
                    c.diamond(x, y, 73., 73.);
                    c.polygon(
                        &[(x, y - 21.), (x + 21., y), (x, y + 21.), (x - 21., y)],
                        "black",
                        None,
                    );
                }
            }
        }
        "HONGANJI" => {
            c.ring(190., 18.);
            c.line(140., 159., 256., 222., 14.);
            c.line(372., 159., 256., 222., 14.);
            for side in [-1., 1.] {
                for j in 0..4 {
                    c.leaf(
                        256. + side * (48. + j as f64 * 24.),
                        216. + j as f64 * 31.,
                        66.,
                        18.,
                        side * (30. + j as f64 * 9.),
                    );
                }
                for j in 0..4 {
                    c.leaf(
                        256. + side * (43. + j as f64 * 23.),
                        243. + j as f64 * 31.,
                        54.,
                        16.,
                        side * (145. - j as f64 * 7.),
                    );
                }
            }
            c.circle(256., 223., 20., "white", None);
        }
        "OUCHI" => {
            c.polygon(
                &[(256., 75.), (437., 256.), (256., 437.), (75., 256.)],
                "none",
                Some(20.),
            );
            for a in [0f64, 90., 180., 270.] {
                let r = a.to_radians();
                c.leaf(256., 256., 121., 42., a);
                c.diamond(256. + 111. * r.sin(), 256. - 111. * r.cos(), 24., 24.);
            }
            c.circle(256., 256., 22., "white", None);
        }
        "AMAGO" => {
            for x in [173., 339.] {
                for y in [173., 339.] {
                    c.rect(x - 64., y - 64., 128., 128., "white");
                    c.rect(x - 19., y - 19., 38., 38., "black");
                }
            }
        }
        "MORI" => {
            c.rect(81., 160., 350., 44., "white");
            for (x, y) in [(256., 264.), (184., 362.), (328., 362.)] {
                c.circle(x, y, 42., "white", None);
            }
        }
        "CHOSOKABE" => {
            c.ring(190., 18.);
            for i in 0..7 {
                let a = i as f64 * 360. / 7.;
                let r = a.to_radians();
                c.leaf(256. + 31. * r.sin(), 256. - 31. * r.cos(), 141., 31., a);
            }
            c.circle(256., 256., 23., "white", None);
        }
        "OTOMO" => {
            c.ring(190., 18.);
            for side in [-1., 1.] {
                c.leaf(256. + side * 33., 347., 173., 57., side * 31.);
                for i in 0..3 {
                    c.leaf(
                        256. + side * (92. + i as f64 * 12.),
                        300. - i as f64 * 48.,
                        66.,
                        20.,
                        side * 75.,
                    );
                }
            }
            c.circle(256., 320., 23., "white", None);
        }
        "SHIMAZU" => {
            c.ring(179., 31.);
            c.rect(232., 119., 48., 274., "white");
            c.rect(119., 232., 274., 48., "white");
        }
        "RYUKYU" => {
            c.ring(190., 18.);
            let mut tail = cubic((285., 158.), (313., 159.), (339., 176.), (337., 204.), 20);
            tail.extend(
                cubic((337., 204.), (335., 224.), (317., 242.), (289., 251.), 20)
                    .into_iter()
                    .skip(1),
            );
            tail.extend(
                cubic((289., 251.), (304., 225.), (308., 205.), (298., 183.), 20)
                    .into_iter()
                    .skip(1),
            );
            tail.extend(
                cubic((298., 183.), (293., 172.), (286., 166.), (285., 158.), 20)
                    .into_iter()
                    .skip(1),
            );
            for a in [0., 120., 240.] {
                let head = rotate(&[(256., 155.)], a, 256., 256.)[0];
                c.circle(head.0, head.1, 48., "white", None);
                c.polygon(
                    &rotate(
                        &tail.iter().map(|&(x, y)| (512. - x, y)).collect::<Vec<_>>(),
                        a,
                        256.,
                        256.,
                    ),
                    "white",
                    None,
                );
            }
        }
        "AINU" => {
            c.ring(190., 18.);
            c.polygon(
                &[
                    (106., 256.),
                    (161., 205.),
                    (338., 202.),
                    (396., 252.),
                    (337., 307.),
                    (162., 307.),
                ],
                "white",
                None,
            );
            c.polygon(&[(114., 256.), (70., 202.), (70., 309.)], "white", None);
            c.polygon(&[(224., 211.), (184., 156.), (282., 204.)], "white", None);
            c.polygon(&[(220., 299.), (185., 355.), (286., 307.)], "white", None);
            c.circle(346., 242., 8., "none", Some(7.));
        }
        _ => bail!("Unknown crest: {name}"),
    };
    Ok(c)
}
pub fn render_svg(path: &Path) -> Result<RgbaImage> {
    render(&xml(path)?)
}
pub fn render(root: &Element) -> Result<RgbaImage> {
    let mut root = root.clone();
    for n in root
        .children
        .iter_mut()
        .filter_map(xmltree::XMLNode::as_mut_element)
    {
        ensure!(
            ["polygon", "circle", "rect", "line"].contains(&n.name.as_str()),
            "unsupported SVG element: {}",
            n.name
        );
        if n.name == "circle" && attr(n, "stroke") == "white" {
            let r: f64 = attr(n, "r").parse()?;
            let width: f64 = attr(n, "stroke-width").parse()?;
            n.attributes
                .insert("r".into(), (r - width / 2.).to_string());
        }
    }
    let mut svg = vec![];
    root.write(&mut svg)?;
    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default())?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(SIZE * SCALE, SIZE * SCALE)
        .context("crest image allocation")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(SCALE as f32, SCALE as f32),
        &mut pixmap.as_mut(),
    );
    let mut image = RgbaImage::new(SIZE * SCALE, SIZE * SCALE);
    for (out, p) in image.pixels_mut().zip(pixmap.pixels()) {
        *out = Rgba([255, 255, 255, p.red()]);
    }
    let mut output = build_art::resize(&image, SIZE, SIZE);
    for pixel in output.pixels_mut() {
        pixel[0] = 255;
        pixel[1] = 255;
        pixel[2] = 255;
    }
    Ok(output)
}
pub fn generate(output: &Path) -> Result<Vec<Value>> {
    fs::create_dir_all(output)?;
    let mut records = vec![];
    for (name, motif) in NAMES.into_iter().zip(MOTIFS) {
        let svg = output.join(format!("{}.svg", name.to_lowercase()));
        let png = svg.with_extension("png");
        xml_write(&svg, &draw(name)?.root)?;
        render_svg(&svg)?.save(&png)?;
        records.push(json!({"civilization":format!("CIVILIZATION_JAC_{name}"),"motif":motif,"method":"original geometric SVG, rasterized by jac-draw-crests","author":"Civ6用万世紀制作チーム","created_utc":chrono::Utc::now().to_rfc3339(),"svg":svg.file_name().unwrap().to_string_lossy(),"png":png.file_name().unwrap().to_string_lossy(),"svg_sha256":sha256(&fs::read(svg)?),"png_sha256":sha256(&fs::read(png)?)}));
    }
    write_json(output.join("generation-log.json"), &json!(records))?;
    println!(
        "Generated {} SVG + PNG crests in {}",
        records.len(),
        output.display()
    );
    Ok(records)
}
