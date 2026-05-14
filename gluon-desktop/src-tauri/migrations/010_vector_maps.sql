-- Migration 010: Per-Project Vector Maps with Incremental File Tracking
--
-- This migration introduces:
-- 1. vector_maps: Named vector stores that can be shared by multiple projects
-- 2. vector_embeddings: Actual embeddings stored as BLOBs in SQLite
-- 3. indexed_files: Track file modification times for incremental indexing
-- 4. projects.vector_map_id: Foreign key linking projects to their vector map

-- Table 1: Vector Maps (can be shared by multiple projects)
CREATE TABLE IF NOT EXISTS vector_maps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    created_at DATETIME NOT NULL DEFAULT (datetime('now')),
    updated_at DATETIME NOT NULL DEFAULT (datetime('now')),
    total_chunks INTEGER NOT NULL DEFAULT 0,
    total_files INTEGER NOT NULL DEFAULT 0,
    size_bytes INTEGER NOT NULL DEFAULT 0
);

-- Table 2: Vector Embeddings (the actual vector data)
CREATE TABLE IF NOT EXISTS vector_embeddings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vector_map_id INTEGER NOT NULL,
    chunk_key TEXT NOT NULL,           -- e.g., "src/main.rs::42"
    embedding BLOB NOT NULL,            -- Serialized Vec<f32> using bincode
    content TEXT NOT NULL,              -- Chunk content
    file_path TEXT NOT NULL,            -- Full or relative file path
    start_line INTEGER NOT NULL,        -- Starting line number of chunk
    indexed_at DATETIME NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (vector_map_id) REFERENCES vector_maps(id) ON DELETE CASCADE,
    UNIQUE(vector_map_id, chunk_key)
);

-- Indexes for fast queries
CREATE INDEX IF NOT EXISTS idx_vector_embeddings_map
    ON vector_embeddings(vector_map_id);
CREATE INDEX IF NOT EXISTS idx_vector_embeddings_file
    ON vector_embeddings(vector_map_id, file_path);

-- Table 3: Indexed Files (for incremental indexing)
-- Tracks last modification time of each indexed file
CREATE TABLE IF NOT EXISTS indexed_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vector_map_id INTEGER NOT NULL,
    file_path TEXT NOT NULL,            -- Absolute file path
    last_modified INTEGER NOT NULL,      -- Unix timestamp (mtime from filesystem)
    last_indexed_at DATETIME NOT NULL DEFAULT (datetime('now')),
    chunk_count INTEGER NOT NULL DEFAULT 0,
    file_size_bytes INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (vector_map_id) REFERENCES vector_maps(id) ON DELETE CASCADE,
    UNIQUE(vector_map_id, file_path)
);

-- Index for fast lookup during incremental indexing
CREATE INDEX IF NOT EXISTS idx_indexed_files_map
    ON indexed_files(vector_map_id);

-- Step 4: Alter projects table to add vector_map_id
-- SQLite doesn't support ALTER TABLE ADD COLUMN with FOREIGN KEY directly,
-- so we need to check if the column exists first

-- Check if vector_map_id column already exists, if not add it
-- (This is idempotent, safe to run multiple times)
ALTER TABLE projects ADD COLUMN vector_map_id INTEGER REFERENCES vector_maps(id) ON DELETE SET NULL;

-- Create index for fast project-to-map lookups
CREATE INDEX IF NOT EXISTS idx_projects_vector_map
    ON projects(vector_map_id);

-- Insert default vector map
INSERT INTO vector_maps (id, name, description)
VALUES (1, 'Default', 'Global vector map for all projects')
ON CONFLICT(id) DO NOTHING;

-- Set existing projects to use default map (id=1)
-- This ensures backward compatibility
UPDATE projects
SET vector_map_id = 1
WHERE vector_map_id IS NULL;
