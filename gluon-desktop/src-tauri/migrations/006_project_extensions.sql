-- Add allowed_extensions to projects table
ALTER TABLE projects ADD COLUMN allowed_extensions TEXT;

-- Extension Templates table
CREATE TABLE extension_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    extensions TEXT NOT NULL -- JSON array of strings
);