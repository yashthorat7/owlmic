//! Generates Rust constants from design/tokens.json and design/copy.json, so the PC uses exactly
//! the colours, sizes and texts the phone and the website use, and turns the Lucide icons in
//! icons/ into drawing commands, so the app has no SVG parser.

use serde_json::{Map, Value};
use std::fmt::Write;
use std::path::PathBuf;

fn main() {
    let design = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../shared/design");
    let tokens = read(&design.join("tokens.json"));
    let copy = read(&design.join("copy.json"));

    let mut out = String::new();
    write_tokens(&mut out, &tokens);
    write_copy(&mut out, &copy);
    write_icons(
        &mut out,
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../shared/assets/icons"),
    );
    write_brand_owl(
        &mut out,
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../shared/assets/brand/icon.svg"),
    );

    let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("design.rs");
    std::fs::write(dest, out).unwrap();
}

fn read(path: &PathBuf) -> Value {
    println!("cargo:rerun-if-changed={}", path.display());
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("can't read {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()))
}

fn object<'a>(v: &'a Value, key: &str) -> &'a Map<String, Value> {
    v[key]
        .as_object()
        .unwrap_or_else(|| panic!("design: `{key}` must be an object"))
}

/// `notFoundHelp` → `NOT_FOUND_HELP`, `status.searching` → `STATUS_SEARCHING`.
fn constant(key: &str) -> String {
    let mut s = String::new();
    for (i, c) in key.chars().enumerate() {
        if c == '.' || c == '-' {
            s.push('_');
        } else if c.is_ascii_uppercase() && i > 0 && !s.ends_with('_') {
            s.push('_');
            s.push(c);
        } else {
            s.push(c.to_ascii_uppercase());
        }
    }
    s
}

fn write_tokens(out: &mut String, tokens: &Value) {
    out.push_str("pub mod color {\n");
    for (name, hex) in object(tokens, "color") {
        let hex = hex.as_str().expect("colours are \"#RRGGBB\" strings");
        let rgb = u32::from_str_radix(hex.trim_start_matches('#'), 16).expect("bad colour");
        writeln!(out, "    pub const {}: u32 = 0x{rgb:06X};", constant(name)).unwrap();
    }
    out.push_str("}\n\npub mod text_style {\n    use crate::TextStyle;\n");
    for (name, style) in object(tokens, "type") {
        writeln!(
            out,
            "    pub const {}: TextStyle = TextStyle {{ size_px: {:?}, line_px: {:?}, weight: {} }};",
            constant(name),
            style["pcPx"].as_f64().unwrap() as f32,
            style["pcLinePx"].as_f64().unwrap() as f32,
            style["weight"].as_u64().unwrap()
        )
        .unwrap();
    }
    out.push_str("}\n");
    for group in ["space", "radius", "motion", "icon", "panel"] {
        writeln!(out, "\npub mod {group} {{").unwrap();
        for (name, value) in object(tokens, group) {
            writeln!(
                out,
                "    pub const {}: f32 = {:?};",
                constant(name),
                value.as_f64().unwrap() as f32
            )
            .unwrap();
        }
        out.push_str("}\n");
    }
}

fn write_copy(out: &mut String, copy: &Value) {
    for (group, module) in [("names", "names"), ("messages", "messages")] {
        writeln!(out, "\npub mod {module} {{").unwrap();
        for (key, text) in object(copy, group) {
            writeln!(
                out,
                "    pub const {}: &str = {:?};",
                constant(key),
                text.as_str().unwrap()
            )
            .unwrap();
        }
        // Lookup by key, for texts named after settings and their values.
        out.push_str(
            "\n    pub fn text(key: &str) -> Option<&'static str> {\n        match key {\n",
        );
        for key in object(copy, group).keys() {
            writeln!(out, "            {key:?} => Some({}),", constant(key)).unwrap();
        }
        out.push_str("            _ => None,\n        }\n    }\n}\n");
    }
}

/// One drawing command, absolute coordinates in the icon's 24 x 24 box.
enum Seg {
    Move(f32, f32),
    Line(f32, f32),
    Cubic([f32; 6]),
    Arc([f32; 3], bool, bool, f32, f32),
    Close,
}

fn write_icons(out: &mut String, dir: &PathBuf) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("icons/ is missing")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "svg"))
        .collect();
    files.sort();
    out.push_str("\npub mod icons {\n    use crate::glyph::{Seg, Shape};\n");
    for file in files {
        let svg = std::fs::read_to_string(&file).unwrap();
        let name = constant(file.file_stem().unwrap().to_str().unwrap());
        let shapes: Vec<String> = elements(&svg)
            .iter()
            .map(|(tag, attrs)| shape(tag, attrs, &file))
            .collect();
        writeln!(
            out,
            "    pub const {name}: &[Shape] = &[{}];",
            shapes.join(", ")
        )
        .unwrap();
    }
    out.push_str("}\n");
}

/// Tags and attributes of the drawing elements in an SVG file.
fn elements(svg: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut found = Vec::new();
    for chunk in svg.split('<').skip(1) {
        let body = chunk.split('>').next().unwrap_or("").trim_end_matches('/');
        let mut parts = body.splitn(2, char::is_whitespace);
        let tag = parts.next().unwrap_or("").to_owned();
        if !["path", "line", "circle", "rect", "polygon", "polyline"].contains(&tag.as_str()) {
            continue;
        }
        let mut attrs = Vec::new();
        let mut rest = parts.next().unwrap_or("");
        while let Some(eq) = rest.find("=\"") {
            let key = rest[..eq].trim().to_owned();
            let after = &rest[eq + 2..];
            let end = after.find('"').unwrap();
            attrs.push((key, after[..end].to_owned()));
            rest = &after[end + 1..];
        }
        found.push((tag, attrs));
    }
    found
}

fn attr(attrs: &[(String, String)], key: &str) -> f32 {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map_or(0.0, |(_, v)| v.parse().unwrap())
}

fn shape(tag: &str, attrs: &[(String, String)], file: &std::path::Path) -> String {
    let a = |k| attr(attrs, k);
    let segs = match tag {
        "circle" => {
            return format!(
                "Shape::Circle {{ c: ({:?}, {:?}), r: {:?} }}",
                a("cx"),
                a("cy"),
                a("r")
            );
        }
        "rect" => {
            return format!(
                "Shape::Rect {{ x: {:?}, y: {:?}, w: {:?}, h: {:?}, r: {:?} }}",
                a("x"),
                a("y"),
                a("width"),
                a("height"),
                a("rx")
            );
        }
        "line" => vec![Seg::Move(a("x1"), a("y1")), Seg::Line(a("x2"), a("y2"))],
        "polygon" | "polyline" => {
            let points = attrs.iter().find(|(k, _)| k == "points").unwrap().1.clone();
            let n: Vec<f32> = points
                .split([' ', ','])
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().unwrap())
                .collect();
            let mut segs: Vec<Seg> = n
                .chunks(2)
                .enumerate()
                .map(|(i, p)| {
                    if i == 0 {
                        Seg::Move(p[0], p[1])
                    } else {
                        Seg::Line(p[0], p[1])
                    }
                })
                .collect();
            if tag == "polygon" {
                segs.push(Seg::Close);
            }
            segs
        }
        _ => {
            let d = &attrs
                .iter()
                .find(|(k, _)| k == "d")
                .unwrap_or_else(|| panic!("{}: path without d", file.display()))
                .1;
            path(d)
        }
    };
    let body: Vec<String> = segs
        .iter()
        .map(|s| match s {
            Seg::Move(x, y) => format!("Seg::Move({x:?}, {y:?})"),
            Seg::Line(x, y) => format!("Seg::Line({x:?}, {y:?})"),
            Seg::Cubic(c) => format!("Seg::Cubic({c:?})"),
            Seg::Arc([rx, ry, rot], large, sweep, x, y) => {
                format!("Seg::Arc {{ radius: ({rx:?}, {ry:?}), rotation: {rot:?}, large: {large}, clockwise: {sweep}, to: ({x:?}, {y:?}) }}")
            }
            Seg::Close => "Seg::Close".to_owned(),
        })
        .collect();
    format!("Shape::Path(&[{}])", body.join(", "))
}

/// Splits path data into commands and numbers; arc flags may be written without separators.
fn tokens(d: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut num = String::new();
    let flush = |num: &mut String, out: &mut Vec<String>| {
        if !num.is_empty() {
            out.push(std::mem::take(num));
        }
    };
    for c in d.chars() {
        if c.is_ascii_alphabetic() && c != 'e' {
            flush(&mut num, &mut out);
            out.push(c.to_string());
        } else if (c == '-' && !num.ends_with('e')) || (c == '.' && num.contains('.')) {
            flush(&mut num, &mut out);
            num.push(c);
        } else if c == ',' || c.is_whitespace() {
            flush(&mut num, &mut out);
        } else {
            num.push(c);
        }
    }
    flush(&mut num, &mut out);
    out
}

fn path(d: &str) -> Vec<Seg> {
    let toks = tokens(d);
    let mut i = 0;
    let (mut x, mut y, mut sx, mut sy) = (0f32, 0f32, 0f32, 0f32);
    // The last cubic's second control point, for S.
    let mut last_ctrl: Option<(f32, f32)> = None;
    let mut cmd = 'M';
    let mut segs = Vec::new();
    let num = |i: &mut usize| -> f32 {
        let v = toks[*i]
            .parse::<f32>()
            .unwrap_or_else(|_| panic!("bad number {:?} in {d}", toks[*i]));
        *i += 1;
        v
    };
    while i < toks.len() {
        if toks[i].chars().all(|c| c.is_ascii_alphabetic()) {
            cmd = toks[i].chars().next().unwrap();
            i += 1;
            if cmd == 'Z' || cmd == 'z' {
                segs.push(Seg::Close);
                (x, y) = (sx, sy);
                last_ctrl = None;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
        let ctrl = last_ctrl.take();
        match cmd.to_ascii_uppercase() {
            'M' => {
                (x, y) = (ox + num(&mut i), oy + num(&mut i));
                (sx, sy) = (x, y);
                segs.push(Seg::Move(x, y));
                // Further pairs after a move are lines.
                cmd = if rel { 'l' } else { 'L' };
            }
            'L' => {
                (x, y) = (ox + num(&mut i), oy + num(&mut i));
                segs.push(Seg::Line(x, y));
            }
            'H' => {
                x = ox + num(&mut i);
                segs.push(Seg::Line(x, y));
            }
            'V' => {
                y = oy + num(&mut i);
                segs.push(Seg::Line(x, y));
            }
            'C' => {
                let c = [
                    ox + num(&mut i),
                    oy + num(&mut i),
                    ox + num(&mut i),
                    oy + num(&mut i),
                    ox + num(&mut i),
                    oy + num(&mut i),
                ];
                (x, y) = (c[4], c[5]);
                last_ctrl = Some((c[2], c[3]));
                segs.push(Seg::Cubic(c));
            }
            'S' => {
                let (c1x, c1y) = ctrl.map_or((x, y), |(cx, cy)| (2.0 * x - cx, 2.0 * y - cy));
                let c = [
                    c1x,
                    c1y,
                    ox + num(&mut i),
                    oy + num(&mut i),
                    ox + num(&mut i),
                    oy + num(&mut i),
                ];
                (x, y) = (c[4], c[5]);
                last_ctrl = Some((c[2], c[3]));
                segs.push(Seg::Cubic(c));
            }
            'Q' => {
                let (qx, qy, ex, ey) = (
                    ox + num(&mut i),
                    oy + num(&mut i),
                    ox + num(&mut i),
                    oy + num(&mut i),
                );
                let c = [
                    x + 2.0 / 3.0 * (qx - x),
                    y + 2.0 / 3.0 * (qy - y),
                    ex + 2.0 / 3.0 * (qx - ex),
                    ey + 2.0 / 3.0 * (qy - ey),
                    ex,
                    ey,
                ];
                (x, y) = (ex, ey);
                segs.push(Seg::Cubic(c));
            }
            'A' => {
                let (rx, ry, rot) = (num(&mut i), num(&mut i), num(&mut i));
                let flag = |t: &str| t == "1";
                let large = flag(&toks[i]);
                i += 1;
                let sweep = flag(&toks[i]);
                i += 1;
                (x, y) = (ox + num(&mut i), oy + num(&mut i));
                segs.push(Seg::Arc([rx, ry, rot], large, sweep, x, y));
            }
            other => panic!("unsupported path command {other} in {d}"),
        }
    }
    segs
}

fn write_brand_owl(out: &mut String, brand_path: &PathBuf) {
    println!("cargo:rerun-if-changed={}", brand_path.display());
    let svg = std::fs::read_to_string(brand_path)
        .unwrap_or_else(|e| panic!("can't read {}: {e}", brand_path.display()));

    let min_x = 24.9f32;
    let min_y = 33.85f32;
    let vb_size = 421.15f32;
    let scale = 24.0 / vb_size;

    let elems = elements(&svg);
    out.push_str("\npub mod brand {\n    use crate::glyph::{Seg, Shape};\n");

    let path_elem = elems
        .iter()
        .find(|(tag, _)| tag == "path")
        .expect("brand icon.svg missing path");
    let d = &path_elem
        .1
        .iter()
        .find(|(k, _)| k == "d")
        .expect("brand icon.svg path missing d")
        .1;
    let raw_segs = path(d);
    let mut normalized_segs = Vec::new();
    for seg in raw_segs {
        match seg {
            Seg::Move(x, y) => normalized_segs.push(format!(
                "Seg::Move({:?}, {:?})",
                (x - min_x) * scale,
                (y - min_y) * scale
            )),
            Seg::Line(x, y) => normalized_segs.push(format!(
                "Seg::Line({:?}, {:?})",
                (x - min_x) * scale,
                (y - min_y) * scale
            )),
            Seg::Cubic(c) => normalized_segs.push(format!(
                "Seg::Cubic([{:?}, {:?}, {:?}, {:?}, {:?}, {:?}])",
                (c[0] - min_x) * scale,
                (c[1] - min_y) * scale,
                (c[2] - min_x) * scale,
                (c[3] - min_y) * scale,
                (c[4] - min_x) * scale,
                (c[5] - min_y) * scale
            )),
            Seg::Close => normalized_segs.push("Seg::Close".to_owned()),
            _ => {}
        }
    }
    writeln!(
        out,
        "    pub const OWL_BODY: &[Seg] = &[{}];",
        normalized_segs.join(", ")
    )
    .unwrap();

    let circles: Vec<String> = elems
        .iter()
        .filter(|(tag, _)| tag == "circle")
        .map(|(_, attrs)| {
            let cx = attr(attrs, "cx");
            let cy = attr(attrs, "cy");
            let r = attr(attrs, "r");
            format!(
                "Shape::Circle {{ c: ({:?}, {:?}), r: {:?} }}",
                (cx - min_x) * scale,
                (cy - min_y) * scale,
                r * scale
            )
        })
        .collect();
    writeln!(
        out,
        "    pub const OWL_PUPILS: &[Shape] = &[{}];",
        circles.join(", ")
    )
    .unwrap();
    out.push_str("}\n");
}
