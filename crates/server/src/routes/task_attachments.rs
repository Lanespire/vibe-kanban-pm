use std::path::PathBuf;

use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    handler::Handler,
    http::{StatusCode, header},
    response::{Json as ResponseJson, Response},
    routing::{delete, get},
};
use chrono::{DateTime, Utc};
use db::models::{
    task::Task,
    task_attachment::{CreateTaskAttachment, TaskAttachment},
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Error as SqlxError;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

const ATTACHMENTS_DIR: &str = "attachments";
const MAX_FILE_SIZE: usize = 50 * 1024 * 1024; // 50MB limit

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TaskAttachmentResponse {
    pub id: Uuid,
    pub task_id: Uuid,
    pub file_name: String,
    pub file_path: String,
    pub mime_type: String,
    pub file_size: i64,
    pub sha256: Option<String>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
    pub download_url: String,
}

impl TaskAttachmentResponse {
    pub fn from_attachment(attachment: TaskAttachment) -> Self {
        let download_url = format!("/api/tasks/{}/attachments/{}/file", attachment.task_id, attachment.id);
        Self {
            id: attachment.id,
            task_id: attachment.task_id,
            file_name: attachment.file_name,
            file_path: attachment.file_path,
            mime_type: attachment.mime_type,
            file_size: attachment.file_size,
            sha256: attachment.sha256,
            created_at: attachment.created_at,
            download_url,
        }
    }
}

/// Get the attachments storage directory
fn get_attachments_dir() -> PathBuf {
    utils::cache_dir().join(ATTACHMENTS_DIR)
}

/// Upload a file attachment to a task
pub async fn upload_task_attachment(
    Path(task_id): Path<Uuid>,
    State(deployment): State<DeploymentImpl>,
    mut multipart: Multipart,
) -> Result<ResponseJson<ApiResponse<TaskAttachmentResponse>>, ApiError> {
    // Verify task exists
    Task::find_by_id(&deployment.db().pool, task_id)
        .await?
        .ok_or(ApiError::Database(SqlxError::RowNotFound))?;

    let attachments_dir = get_attachments_dir();
    fs::create_dir_all(&attachments_dir).await?;

    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("file") {
            let file_name = field
                .file_name()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "attachment".to_string());

            let content_type = field
                .content_type()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());

            let data = field.bytes().await?;
            let file_size = data.len() as i64;

            // Calculate SHA256 hash
            let mut hasher = Sha256::new();
            hasher.update(&data);
            let hash = format!("{:x}", hasher.finalize());

            // Create unique file path
            let extension = std::path::Path::new(&file_name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            let stored_name = if extension.is_empty() {
                format!("{}", Uuid::new_v4())
            } else {
                format!("{}.{}", Uuid::new_v4(), extension)
            };
            let file_path = attachments_dir.join(&stored_name);

            // Write file to disk
            let mut file = File::create(&file_path).await?;
            file.write_all(&data).await?;
            file.flush().await?;

            // Create database record
            let attachment = TaskAttachment::create(
                &deployment.db().pool,
                &CreateTaskAttachment {
                    task_id,
                    file_name: file_name.clone(),
                    file_path: stored_name,
                    mime_type: content_type,
                    file_size,
                    sha256: Some(hash),
                },
            )
            .await?;

            return Ok(ResponseJson(ApiResponse::success(
                TaskAttachmentResponse::from_attachment(attachment),
            )));
        }
    }

    Err(ApiError::BadRequest("No file field found in request".to_string()))
}

/// List all attachments for a task
pub async fn list_task_attachments(
    Path(task_id): Path<Uuid>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<TaskAttachmentResponse>>>, ApiError> {
    let attachments = TaskAttachment::find_by_task_id(&deployment.db().pool, task_id).await?;
    let responses: Vec<TaskAttachmentResponse> = attachments
        .into_iter()
        .map(TaskAttachmentResponse::from_attachment)
        .collect();
    Ok(ResponseJson(ApiResponse::success(responses)))
}

/// Download an attachment file
pub async fn download_task_attachment(
    Path((task_id, attachment_id)): Path<(Uuid, Uuid)>,
    State(deployment): State<DeploymentImpl>,
) -> Result<Response, ApiError> {
    let attachment = TaskAttachment::find_by_id(&deployment.db().pool, attachment_id)
        .await?
        .ok_or(ApiError::Database(SqlxError::RowNotFound))?;

    // Verify the attachment belongs to this task
    if attachment.task_id != task_id {
        return Err(ApiError::BadRequest("Attachment does not belong to this task".to_string()));
    }

    let file_path = get_attachments_dir().join(&attachment.file_path);

    let file = File::open(&file_path).await?;
    let metadata = file.metadata().await?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, &attachment.mime_type)
        .header(header::CONTENT_LENGTH, metadata.len())
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", attachment.file_name),
        )
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .body(body)
        .map_err(|e| ApiError::BadRequest(format!("Failed to build response: {}", e)))?;

    Ok(response)
}

/// Delete an attachment
pub async fn delete_task_attachment(
    Path((task_id, attachment_id)): Path<(Uuid, Uuid)>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let attachment = TaskAttachment::find_by_id(&deployment.db().pool, attachment_id)
        .await?
        .ok_or(ApiError::Database(SqlxError::RowNotFound))?;

    // Verify the attachment belongs to this task
    if attachment.task_id != task_id {
        return Err(ApiError::BadRequest("Attachment does not belong to this task".to_string()));
    }

    // Delete file from disk
    let file_path = get_attachments_dir().join(&attachment.file_path);
    if file_path.exists() {
        fs::remove_file(&file_path).await?;
    }

    // Delete from database
    TaskAttachment::delete(&deployment.db().pool, attachment_id).await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

pub fn routes() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/{task_id}/attachments",
            get(list_task_attachments)
                .post(upload_task_attachment.layer(DefaultBodyLimit::max(MAX_FILE_SIZE))),
        )
        .route(
            "/{task_id}/attachments/{attachment_id}",
            delete(delete_task_attachment),
        )
        .route(
            "/{task_id}/attachments/{attachment_id}/file",
            get(download_task_attachment),
        )
}
