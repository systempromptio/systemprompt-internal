//! The render pipeline: sheet data in, PDF and page images out.
//!
//! The split is deliberate. Everything that decides what the page *says* —
//! loading sheet data, numbering sections, inlining assets, applying the
//! template — happens here in Rust. The subprocess at the end does one thing:
//! turn a finished, self-contained HTML string into a PDF. `WeasyPrint`'s
//! layout engine is Python and has no Rust binding, so that boundary has to
//! exist; it is kept as thin as it can be so that almost nothing about a
//! factsheet is defined outside this crate.

use handlebars::Handlebars;
use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::assets::Assets;
use crate::error::{FactsheetError, FactsheetResult};
use crate::model::FactsheetDoc;
use crate::template_data::{RenderReport, css_string, inline_helper, lines_helper, prepare_blocks};

const MAIN_TEMPLATE: &str = "factsheet";

/// What a completed render produced.
#[derive(Debug, Clone)]
pub struct RenderedFactsheet {
    pub id: String,
    pub pdf_path: PathBuf,
    // Why: One PNG per page, in page order. These are the preview surface — the
    // skill shows them, the reader reacts, the data changes, and the sheet is
    // rendered again.
    pub page_images: Vec<PathBuf>,
    pub page_count: usize,
}

/// Where the engine's inputs live on disk.
#[derive(Debug, Clone)]
pub struct EnginePaths {
    // Why: `storage/files/factsheet`
    pub root: PathBuf,
    // Why: The `factsheet-render.py` sidecar.
    pub script: PathBuf,
    // Why: Python interpreter. A venv path in a deployed image, `python3` locally.
    pub python: PathBuf,
}

impl EnginePaths {
    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }

    pub fn templates_dir(&self) -> PathBuf {
        self.root.join("templates")
    }

    pub fn sheets_dir(&self) -> PathBuf {
        self.root.join("sheets")
    }
}

#[derive(Debug)]
pub struct FactsheetEngine {
    paths: EnginePaths,
    assets: Assets,
    registry: Handlebars<'static>,
    base_css: String,
}

impl FactsheetEngine {
    pub fn new(paths: EnginePaths) -> FactsheetResult<Self> {
        let assets = Assets::new(paths.assets_dir());
        let templates = paths.templates_dir();

        let mut registry = Handlebars::new();
        // Why: a missing block partial must fail the render rather than emit a
        // silently blank section. A factsheet that quietly loses its call to
        // action is worse than one that does not build.
        registry.set_strict_mode(true);
        registry.register_helper("inline", Box::new(inline_helper));
        registry.register_helper("lines", Box::new(lines_helper));

        registry
            .register_template_file(MAIN_TEMPLATE, templates.join("factsheet.hbs"))
            .map_err(|e| FactsheetError::Template(e.to_string()))?;

        let partials = templates.join("partials");
        let entries = std::fs::read_dir(&partials).map_err(|source| FactsheetError::Io {
            path: partials.display().to_string(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| FactsheetError::Io {
                path: partials.display().to_string(),
                source,
            })?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "hbs") {
                // Why: The partial's file stem is the block's serde tag; that is how
                // the template dispatches, so the two cannot drift apart.
                let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                registry
                    .register_template_file(name, &path)
                    .map_err(|e| FactsheetError::Template(e.to_string()))?;
            }
        }

        let base_css_path = templates.join("base.css");
        let base_css =
            std::fs::read_to_string(&base_css_path).map_err(|source| FactsheetError::Io {
                path: base_css_path.display().to_string(),
                source,
            })?;

        Ok(Self {
            paths,
            assets,
            registry,
            base_css,
        })
    }

    pub fn list_sheets(&self) -> FactsheetResult<Vec<String>> {
        let dir = self.paths.sheets_dir();
        let entries = std::fs::read_dir(&dir).map_err(|source| FactsheetError::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let mut ids: Vec<String> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "yaml"))
            .filter_map(|path| {
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .map(str::to_owned)
            })
            .collect();
        ids.sort();
        Ok(ids)
    }

    pub fn load_sheet(&self, id: &str) -> FactsheetResult<FactsheetDoc> {
        // Why: an id is a filename here. Anything with a separator in it would
        // read outside the sheets directory.
        if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
            return Err(FactsheetError::SheetMissing(id.to_owned()));
        }
        let path = self.paths.sheets_dir().join(format!("{id}.yaml"));
        let raw = std::fs::read_to_string(&path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                FactsheetError::SheetMissing(id.to_owned())
            } else {
                FactsheetError::Io {
                    path: path.display().to_string(),
                    source,
                }
            }
        })?;
        serde_yaml::from_str(&raw).map_err(|e| FactsheetError::Parse {
            id: id.to_owned(),
            message: e.to_string(),
        })
    }

    // Why: Apply the template. The result is self-contained: fonts and diagrams are
    // embedded, so it needs neither a network nor a base URL.
    pub fn render_html(&self, doc: &FactsheetDoc) -> FactsheetResult<String> {
        let mut doc = doc.clone();
        doc.number_sections();

        let diagram_svg = match doc.diagram.as_deref() {
            Some(name) => self.assets.load_svg(name)?,
            None => String::new(),
        };

        let mut data = serde_json::to_value(&doc).map_err(|e| FactsheetError::Parse {
            id: doc.id.clone(),
            message: e.to_string(),
        })?;
        prepare_blocks(&mut data);
        if let Some(object) = data.as_object_mut() {
            object.insert("base_css".to_owned(), self.base_css.clone().into());
            object.insert("font_face".to_owned(), self.assets.font_face_css()?.into());
            object.insert("logo".to_owned(), self.assets.logo_on_light()?.into());
            object.insert("diagram_svg".to_owned(), diagram_svg.into());
            object.insert("footer_css".to_owned(), css_string(&doc.footer).into());
        }

        self.registry
            .render(MAIN_TEMPLATE, &data)
            .map_err(|e| FactsheetError::Template(e.to_string()))
    }

    // Why: Render to PDF plus one PNG per page, writing into `out_dir`.
    //
    // Fails when the sheet overruns its page budget — see
    // [`FactsheetError::PageBudget`].
    pub async fn render_pdf(
        &self,
        doc: &FactsheetDoc,
        out_dir: &Path,
    ) -> FactsheetResult<RenderedFactsheet> {
        let html = self.render_html(doc)?;

        tokio::fs::create_dir_all(out_dir)
            .await
            .map_err(|source| FactsheetError::Io {
                path: out_dir.display().to_string(),
                source,
            })?;

        let pdf_path = out_dir.join(format!("{}.pdf", doc.id));
        let report = self
            .run_renderer(&html, &pdf_path, out_dir, &doc.id)
            .await?;

        if report.page_count > doc.max_pages {
            return Err(FactsheetError::PageBudget {
                id: doc.id.clone(),
                pages: report.page_count,
                max: doc.max_pages,
            });
        }

        Ok(RenderedFactsheet {
            id: doc.id.clone(),
            pdf_path,
            page_images: report.page_images.into_iter().map(PathBuf::from).collect(),
            page_count: report.page_count,
        })
    }

    async fn run_renderer(
        &self,
        html: &str,
        pdf_path: &Path,
        out_dir: &Path,
        id: &str,
    ) -> FactsheetResult<RenderReport> {
        use tokio::io::AsyncWriteExt as _;

        let mut child = Command::new(&self.paths.python)
            .arg(&self.paths.script)
            .arg("--pdf")
            .arg(pdf_path)
            .arg("--png-dir")
            .arg(out_dir)
            .arg("--png-prefix")
            .arg(id)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                FactsheetError::Renderer(format!(
                    "could not start {} {}: {e}",
                    self.paths.python.display(),
                    self.paths.script.display()
                ))
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(html.as_bytes())
                .await
                .map_err(|e| FactsheetError::Renderer(format!("writing HTML to renderer: {e}")))?;
            stdin
                .shutdown()
                .await
                .map_err(|e| FactsheetError::Renderer(format!("closing renderer stdin: {e}")))?;
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| FactsheetError::Renderer(e.to_string()))?;

        if !output.status.success() {
            return Err(FactsheetError::Renderer(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }

        serde_json::from_slice(&output.stdout).map_err(|e| {
            FactsheetError::Renderer(format!(
                "renderer returned unreadable output ({e}): {}",
                String::from_utf8_lossy(&output.stdout).trim()
            ))
        })
    }
}
