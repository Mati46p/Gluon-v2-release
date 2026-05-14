-- Add .ini extension support to all existing projects
-- This migration ensures that .ini files (like pytest.ini) are visible in file trees

-- Update all projects that have allowed_extensions set to include 'ini' if not already present
UPDATE projects
SET allowed_extensions = (
    SELECT json_insert(
        COALESCE(allowed_extensions, '[]'),
        '$[#]',
        'ini'
    )
)
WHERE allowed_extensions IS NOT NULL
  AND allowed_extensions != ''
  AND allowed_extensions NOT LIKE '%"ini"%';

-- Note: Projects with NULL or empty allowed_extensions will automatically use
-- the default ALLOWED_EXTENSIONS list from main.rs, which already includes 'ini'
