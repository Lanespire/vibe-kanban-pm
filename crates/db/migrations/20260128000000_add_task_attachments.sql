-- Add file attachments to tasks (not just images)
-- This allows attaching PDFs, documents, design files, etc. to tasks

CREATE TABLE task_attachments (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    file_name TEXT NOT NULL,
    file_path TEXT NOT NULL,  -- Relative path to stored file
    mime_type TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    sha256 TEXT,  -- Optional hash for integrity verification and deduplication
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for faster lookups
CREATE INDEX idx_task_attachments_task_id ON task_attachments(task_id);
CREATE INDEX idx_task_attachments_sha256 ON task_attachments(sha256);
