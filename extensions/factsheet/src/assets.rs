//! Asset loading and inlining.
//!
//! Everything a rendered sheet needs is embedded in the HTML before it reaches
//! the renderer: fonts as base64 data URIs, SVGs as literal markup. That is not
//! an optimisation, it is a requirement — the renderer is handed a string with
//! no network and no reliable base URL.
//!
//! # Why the SVG surgery is string work
//!
//! `WeasyPrint` does not apply CSS to elements *inside* an inline `<svg>`. A
//! `fill="currentColor"` on a path will not pick up the surrounding `color`,
//! and a stylesheet rule targeting an SVG child does nothing. So every colour
//! and size variation has to be baked into the markup before render. This is
//! the single constraint that shapes this module.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use std::path::{Path, PathBuf};

use crate::error::{FactsheetError, FactsheetResult};

/// Ink used for provider wordmarks on light paper.
const MARK_INK: &str = "#1a2230";

/// The wordmark fill in `logo-dark.svg`, swapped out for the dark masthead.
const LOGO_INK: &str = "#0F172A";

#[derive(Debug, Clone)]
pub struct Assets {
    root: PathBuf,
}

impl Assets {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The three `@font-face` rules the design system actually uses.
    pub fn font_face_css(&self) -> FactsheetResult<String> {
        let fonts = self.root.join("fonts");
        let inter_regular = font_data_uri(&fonts.join("Inter-Regular.woff2"), FontFormat::Woff2)?;
        let inter_bold = font_data_uri(&fonts.join("Inter-Bold.woff2"), FontFormat::Woff2)?;
        let archivo = font_data_uri(&fonts.join("ArchivoBlack-Regular.ttf"), FontFormat::TrueType)?;

        Ok(format!(
            "@font-face {{ font-family: 'Inter'; font-style: normal; font-weight: 400; src: \
             {inter_regular}; }}\n@font-face {{ font-family: 'Inter'; font-style: normal; \
             font-weight: 700; src: {inter_bold}; }}\n@font-face {{ font-family: 'ArchivoBlack'; \
             font-style: normal; font-weight: 400; src: {archivo}; }}\n"
        ))
    }

    /// Read an SVG and strip the XML declaration and doctype, which are illegal
    /// once the markup is inlined into an HTML document.
    pub fn load_svg(&self, name: &str) -> FactsheetResult<String> {
        let path = self.root.join(name);
        let raw = std::fs::read_to_string(&path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                FactsheetError::AssetMissing(name.to_owned())
            } else {
                FactsheetError::Io {
                    path: path.display().to_string(),
                    source,
                }
            }
        })?;
        Ok(strip_prolog(&raw))
    }

    /// Black wordmark with the orange mark — for use on white paper.
    pub fn logo_on_light(&self) -> FactsheetResult<String> {
        self.load_svg("logo-dark.svg")
    }

    /// White wordmark with the orange mark — for use on a dark ground.
    pub fn logo_on_dark(&self) -> FactsheetResult<String> {
        Ok(self.logo_on_light()?.replace(
            &format!("fill=\"{LOGO_INK}\""),
            "fill=\"#FFFFFF\"",
        ))
    }

    /// A provider wordmark, recoloured to ink and sized to fill its tile.
    pub fn brand_mark(&self, name: &str) -> FactsheetResult<String> {
        let svg = self.load_svg(name)?;
        // Claude ships a white wordmark; the others carry no fill at all and
        // default to black, so they take the fill on the root instead.
        let inked = if svg.contains("fill=\"#ffffff\"") {
            svg.replace("fill=\"#ffffff\"", &format!("fill=\"{MARK_INK}\""))
        } else {
            set_root_attr(&svg, &format!("fill=\"{MARK_INK}\""))
        };
        Ok(set_svg_size(&inked, "100%", "100%"))
    }
}

#[derive(Debug, Clone, Copy)]
enum FontFormat {
    Woff2,
    TrueType,
}

impl FontFormat {
    const fn mime(self) -> &'static str {
        match self {
            Self::Woff2 => "font/woff2",
            Self::TrueType => "font/ttf",
        }
    }

    const fn css_format(self) -> &'static str {
        match self {
            Self::Woff2 => "woff2",
            Self::TrueType => "truetype",
        }
    }
}

fn font_data_uri(path: &Path, format: FontFormat) -> FactsheetResult<String> {
    let bytes = std::fs::read(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            FactsheetError::AssetMissing(path.display().to_string())
        } else {
            FactsheetError::Io {
                path: path.display().to_string(),
                source,
            }
        }
    })?;
    let encoded = STANDARD.encode(&bytes);
    Ok(format!(
        "url(data:{};base64,{encoded}) format('{}')",
        format.mime(),
        format.css_format()
    ))
}

/// Drop a leading `<?xml ... ?>` and any `<!DOCTYPE ...>`.
fn strip_prolog(raw: &str) -> String {
    let mut out = raw.trim_start();
    if out.starts_with("<?xml")
        && let Some(end) = out.find("?>")
    {
        out = out[end + 2..].trim_start();
    }
    if let Some(start) = out.find("<!DOCTYPE")
        && let Some(len) = out[start..].find('>')
    {
        let mut owned = String::with_capacity(out.len());
        owned.push_str(&out[..start]);
        owned.push_str(&out[start + len + 1..]);
        return owned.trim().to_owned();
    }
    out.trim().to_owned()
}

/// Insert an attribute into the root `<svg>` tag.
fn set_root_attr(svg: &str, attr: &str) -> String {
    svg.replacen("<svg", &format!("<svg {attr}"), 1)
}

/// Force explicit dimensions on the root `<svg>`, dropping any it already
/// carries. Bounded to the root tag: a `width=` on a child rect is geometry,
/// not sizing, and removing it silently corrupts the drawing.
fn set_svg_size(svg: &str, width: &str, height: &str) -> String {
    let Some(open) = svg.find("<svg") else {
        return svg.to_owned();
    };
    let Some(close_offset) = svg[open..].find('>') else {
        return svg.to_owned();
    };
    let close = open + close_offset;

    let root_tag = &svg[open..close];
    let cleaned = strip_attrs(root_tag, &["width", "height", "preserveAspectRatio"]);

    format!(
        "{}{cleaned} width=\"{width}\" height=\"{height}\" \
         preserveAspectRatio=\"xMidYMid meet\"{}",
        &svg[..open],
        &svg[close..]
    )
}

/// Remove `name="..."` attributes from a single tag's source.
fn strip_attrs(tag: &str, names: &[&str]) -> String {
    let mut out = tag.to_owned();
    for name in names {
        let needle = format!(" {name}=\"");
        while let Some(start) = out.find(&needle) {
            let value_start = start + needle.len();
            let Some(value_len) = out[value_start..].find('"') else {
                break;
            };
            out.replace_range(start..=(value_start + value_len), "");
        }
    }
    out
}
