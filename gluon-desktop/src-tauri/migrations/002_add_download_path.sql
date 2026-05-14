-- Add download_path setting with default value (empty string means use project directory)
INSERT INTO settings (key, value) VALUES ('download_path', '') 
ON CONFLICT(key) DO NOTHING;