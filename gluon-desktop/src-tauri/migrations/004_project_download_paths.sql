-- Add download_path to projects table
ALTER TABLE projects ADD COLUMN download_path TEXT DEFAULT NULL;

-- For existing projects, set download_path to the project's root directory
UPDATE projects SET download_path = path;