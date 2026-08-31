//! Persisting a rendered factsheet.
//!
//! The PDF and its page previews go into the `files` domain: bytes on disk
//! under the storage root, one row each carrying mime type, size, checksum and
//! the originating context, and a public URL to reach them by.
//!
//! They are deliberately *not* inlined into the artifact. Artifact file parts
//! are base64 TEXT with no size guard and no populated URI, so a PDF stored
//! there bloats the row and cannot be linked to. A file row and a URL is the
//! right shape for a binary of this size.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use std::path::Path;
use systemprompt::database::DbPool;
use systemprompt::files::{
    FileRepository, FileUploadRequestBuilder, FileUploadService, FilesConfig,
};
use systemprompt::models::execution::context::RequestContext;
use systemprompt_factsheet::RenderedFactsheet;

use crate::error::{ServerError, ServerResult};

/// A stored artefact: where it landed and how to reach it.
#[derive(Debug, Clone)]
pub struct StoredFile {
    pub name: String,
    pub public_url: String,
    pub size_bytes: i64,
}

/// Everything one render produced, once persisted.
#[derive(Debug, Clone)]
pub struct StoredFactsheet {
    pub pdf: StoredFile,
    pub pages: Vec<StoredFile>,
    pub page_count: usize,
}

pub async fn store(
    db_pool: &DbPool,
    files_config: &FilesConfig,
    rendered: &RenderedFactsheet,
    ctx: &RequestContext,
) -> ServerResult<StoredFactsheet> {
    let repository =
        FileRepository::new(db_pool).map_err(|e| ServerError::Storage(e.to_string()))?;
    let service = FileUploadService::new(repository, files_config.clone());

    if !service.is_enabled() {
        return Err(ServerError::Storage(
            "File persistence is disabled, so a rendered factsheet cannot be stored. Enable \
             uploads in services/config/files.yaml."
                .to_owned(),
        ));
    }

    let pdf = upload(
        &service,
        ctx,
        &rendered.pdf_path,
        "application/pdf",
        &format!("{}-factsheet.pdf", rendered.id),
    )
    .await?;

    let mut pages = Vec::with_capacity(rendered.page_images.len());
    for (index, image) in rendered.page_images.iter().enumerate() {
        pages.push(
            upload(
                &service,
                ctx,
                image,
                "image/png",
                &format!("{}-factsheet-p{}.png", rendered.id, index + 1),
            )
            .await?,
        );
    }

    Ok(StoredFactsheet {
        pdf,
        pages,
        page_count: rendered.page_count,
    })
}

async fn upload(
    service: &FileUploadService,
    ctx: &RequestContext,
    path: &Path,
    mime_type: &str,
    name: &str,
) -> ServerResult<StoredFile> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| ServerError::Storage(format!("reading {}: {e}", path.display())))?;

    let request =
        FileUploadRequestBuilder::new(mime_type, STANDARD.encode(&bytes), ctx.context_id().clone())
            .with_name(name)
            .with_user_id(ctx.user_id().clone())
            .with_session_id(ctx.session_id().clone())
            .with_trace_id(ctx.trace_id().clone())
            .build();

    let uploaded = service
        .upload_file(request)
        .await
        .map_err(|e| ServerError::Storage(e.to_string()))?;

    Ok(StoredFile {
        name: name.to_owned(),
        public_url: uploaded.public_url,
        size_bytes: uploaded.size_bytes,
    })
}
