//! Embeds the optional Apple build input as `apple_symbols::APPLE_SYMBOLS` and `SF_PRO_TEXT`.
//!
//! `APPLE_ASSETS_DIR=<dir>` (default `<workspace>/apple-fonts`, git-ignored) holds a `symbols/`
//! folder of SF Symbols SVG exports, and the SF Pro Text fonts (`SF-Pro-Text-Regular.otf`,
//! `-Medium`, `-Semibold`, in it or in a `San-Francisco-Pro-Fonts-master/` folder). `apple-symbols.txt` says which symbol draws which of
//! KreativPdf's icons. Each is made square and tintable here and embedded under the icon's name.
//! Without the folder the table is empty and every icon keeps its Lucide drawing. Nothing from it
//! is ever committed (AGENTS.md §1.6).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

/// A symbol larger than this is not read: a file in the folder is still untrusted input.
const MAX_BYTES: u64 = 256 << 10;

fn main() {
    println!("cargo::rerun-if-env-changed=APPLE_ASSETS_DIR");
    println!("cargo::rerun-if-changed=apple-symbols.txt");
    println!("cargo::rerun-if-changed=build.rs");
    let Some(out_dir) = env::var_os("OUT_DIR").map(PathBuf::from) else { return };
    let base = assets_base();
    let symbols_dir = base.as_ref().map(|b| b.join("symbols")).filter(|d| d.is_dir());
    if let Some(dir) = &symbols_dir {
        println!("cargo::rerun-if-changed={}", dir.display());
    }
    let mut table = String::from("pub static APPLE_SYMBOLS: &[(&str, &[u8])] = &[\n");
    let mut count = 0usize;
    if let Some(dir) = symbols_dir
        && let Ok(map) = fs::read_to_string("apple-symbols.txt")
    {
        let _ = fs::create_dir_all(out_dir.join("symbols"));
        for line in map.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let Some((icon, symbol)) = line.split_once(' ') else { continue };
            let (icon, symbol) = (icon.trim(), symbol.trim());
            if !plain(icon) || !plain(symbol) {
                println!("cargo::warning=apple-symbols.txt: skipped {line:?}");
                continue;
            }
            let Some(svg) = read_symbol(&dir.join(format!("{symbol}.svg"))).and_then(|s| square(&s)) else { continue };
            let path = out_dir.join("symbols").join(format!("{icon}.svg"));
            if fs::write(&path, svg).is_ok() {
                let _ = writeln!(table, "    ({icon:?}, include_bytes!({:?})),", path.display().to_string());
                count += 1;
            }
        }
    }
    table.push_str("];\n");
    table.push_str(&sf_pro_text(base.as_deref()));
    if fs::write(out_dir.join("apple_symbols.rs"), table).is_err() {
        println!("cargo::error=could not write apple_symbols.rs");
    }
    if count == 0 {
        println!("cargo::warning=building without Apple symbols: set APPLE_ASSETS_DIR to a folder with symbols/*.svg");
    }
}

/// An icon or symbol name: letters, digits, dots, dashes, and nothing that can leave the folder.
fn plain(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

/// The folder of the Apple input: `APPLE_ASSETS_DIR`, else `<workspace>/apple-fonts`.
fn assets_base() -> Option<PathBuf> {
    let base = env::var_os("APPLE_ASSETS_DIR").map(PathBuf::from).or_else(|| {
        let manifest = env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from)?;
        Some(manifest.join("..").join("..").join("apple-fonts"))
    })?;
    base.is_dir().then_some(base)
}

/// `SF_PRO_TEXT`: the Regular, Medium and Semibold faces when all three are in the folder, else
/// empty (a family with a missing weight would shift every heading).
fn sf_pro_text(base: Option<&Path>) -> String {
    const FACES: [&str; 3] = ["Regular", "Medium", "Semibold"];
    let mut found = Vec::new();
    if let Some(base) = base {
        for dir in [base.join("San-Francisco-Pro-Fonts-master"), base.to_path_buf()] {
            let paths: Vec<PathBuf> = FACES.iter().map(|w| dir.join(format!("SF-Pro-Text-{w}.otf"))).collect();
            if paths.iter().all(|p| fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() < (16 << 20))) {
                found = paths;
                break;
            }
        }
    }
    let mut out = String::from("pub static SF_PRO_TEXT: &[(&str, &[u8])] = &[\n");
    for (weight, path) in FACES.iter().zip(&found) {
        let _ = writeln!(out, "    (\"SF-Pro-Text-{weight}\", include_bytes!({:?})),", path.display().to_string());
    }
    out.push_str("];\n");
    out
}

fn read_symbol(path: &Path) -> Option<String> {
    if fs::metadata(path).ok()?.len() > MAX_BYTES {
        return None;
    }
    fs::read_to_string(path).ok()
}

/// The number after `key` in `svg` (`width='60.26px'`).
fn number_after(svg: &str, key: &str) -> Option<f64> {
    let rest = svg.get(svg.find(key)? + key.len()..)?;
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))?;
    rest.get(..end)?.parse().ok().filter(|n: &f64| n.is_finite() && *n > 0.0 && *n < 10_000.0)
}

/// The box round the points of the paths in `body`, in the export's own coordinates: `x,y` pairs
/// (`Q 11.2,14.1 9.1,16.2`). `None` when there are none.
fn bounds(body: &str) -> Option<(f64, f64, f64, f64)> {
    let mut b: Option<(f64, f64, f64, f64)> = None;
    for token in body.split_whitespace() {
        let Some((x, y)) = token.split_once(',') else { continue };
        let (Ok(x), Ok(y)) = (x.parse::<f64>(), y.parse::<f64>()) else { continue };
        if !(x.is_finite() && y.is_finite()) {
            continue;
        }
        b = Some(match b {
            None => (x, y, x, y),
            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
        });
    }
    b
}

/// An SF Symbols export made a square SVG around the symbol itself (the export leaves a margin
/// that differs from symbol to symbol), drawn in `currentColor` with no outline, so the icon
/// loader can tint it. The symbol fills about five sixths of the square.
fn square(svg: &str) -> Option<String> {
    let (w, h) = (number_after(svg, "width='")?, number_after(svg, "height='")?);
    let body = svg.get(svg.find("<g")?..svg.rfind("</svg>")?)?;
    let body = body.replace("fill='black'", "fill='currentColor'").replace("stroke='black'", "stroke='none'");
    // The group flips the paths: y on screen is the export's height minus y.
    let (x0, y0, x1, y1) = bounds(&body).unwrap_or((0.0, 0.0, w, h));
    let (top, bottom) = (h - y1, h - y0);
    let side = ((x1 - x0).max(bottom - top) * 1.2).max(1.0);
    let (vx, vy) = ((x0 + x1) / 2.0 - side / 2.0, (top + bottom) / 2.0 - side / 2.0);
    Some(format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='{vx} {vy} {side} {side}'>{body}</svg>"))
}
