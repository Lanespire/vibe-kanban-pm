use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use ts_rs::TS;
use uuid::Uuid;

/// File attachment for a task (any file type, not just images)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct TaskAttachment {
    pub id: Uuid,
    pub task_id: Uuid,
    pub file_name: String,
    pub file_path: String,
    pub mime_type: String,
    pub file_size: i64,
    pub sha256: Option<String>,
    #[ts(type = "Date")]
    pub created_at: DateTime<Utc>,
}

/// Data for creating a new task attachment
#[derive(Debug, Clone, Deserialize, TS)]
pub struct CreateTaskAttachment {
    pub task_id: Uuid,
    pub file_name: String,
    pub file_path: String,
    pub mime_type: String,
    pub file_size: i64,
    pub sha256: Option<String>,
}

impl TaskAttachment {
    /// Create a new task attachment
    pub async fn create(pool: &SqlitePool, data: &CreateTaskAttachment) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query_as!(
            TaskAttachment,
            r#"INSERT INTO task_attachments (id, task_id, file_name, file_path, mime_type, file_size, sha256)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id as "id!: Uuid",
                         task_id as "task_id!: Uuid",
                         file_name as "file_name!",
                         file_path as "file_path!",
                         mime_type as "mime_type!",
                         file_size as "file_size!",
                         sha256,
                         created_at as "created_at!: DateTime<Utc>""#,
            id,
            data.task_id,
            data.file_name,
            data.file_path,
            data.mime_type,
            data.file_size,
            data.sha256,
        )
        .fetch_one(pool)
        .await
    }

    /// Find all attachments for a task
    pub async fn find_by_task_id(pool: &SqlitePool, task_id: Uuid) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            TaskAttachment,
            r#"SELECT id as "id!: Uuid",
                      task_id as "task_id!: Uuid",
                      file_name as "file_name!",
                      file_path as "file_path!",
                      mime_type as "mime_type!",
                      file_size as "file_size!",
                      sha256,
                      created_at as "created_at!: DateTime<Utc>"
               FROM task_attachments
               WHERE task_id = $1
               ORDER BY created_at"#,
            task_id
        )
        .fetch_all(pool)
        .await
    }

    /// Find attachment by ID
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            TaskAttachment,
            r#"SELECT id as "id!: Uuid",
                      task_id as "task_id!: Uuid",
                      file_name as "file_name!",
                      file_path as "file_path!",
                      mime_type as "mime_type!",
                      file_size as "file_size!",
                      sha256,
                      created_at as "created_at!: DateTime<Utc>"
               FROM task_attachments
               WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    /// Find attachment by SHA256 hash (for deduplication)
    pub async fn find_by_sha256(pool: &SqlitePool, sha256: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            TaskAttachment,
            r#"SELECT id as "id!: Uuid",
                      task_id as "task_id!: Uuid",
                      file_name as "file_name!",
                      file_path as "file_path!",
                      mime_type as "mime_type!",
                      file_size as "file_size!",
                      sha256,
                      created_at as "created_at!: DateTime<Utc>"
               FROM task_attachments
               WHERE sha256 = $1"#,
            sha256
        )
        .fetch_optional(pool)
        .await
    }

    /// Delete an attachment by ID
    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(r#"DELETE FROM task_attachments WHERE id = $1"#, id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Delete all attachments for a task
    pub async fn delete_by_task_id(pool: &SqlitePool, task_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(r#"DELETE FROM task_attachments WHERE task_id = $1"#, task_id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
