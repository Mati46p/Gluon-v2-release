--- Add .xsd and .wsdl extension support to all existing projects
--- This migration ensures that XSD (XML Schema) and WSDL (Web Services Description) files are visible in file trees

-- Update all projects that have allowed_extensions set to include 'xsd' if not already present
UPDATE projects
SET allowed_extensions = (
    SELECT json_insert(
        COALESCE(allowed_extensions, '[]'),
        '$[#]',
        'xsd'
    )
)
WHERE allowed_extensions IS NOT NULL
  AND allowed_extensions != ''
  AND allowed_extensions NOT LIKE '%"xsd"%';

-- Update all projects that have allowed_extensions set to include 'wsdl' if not already present
UPDATE projects
SET allowed_extensions = (
    SELECT json_insert(
        COALESCE(allowed_extensions, '[]'),
        '$[#]',
        'wsdl'
    )
)
WHERE allowed_extensions IS NOT NULL
  AND allowed_extensions != ''
  AND allowed_extensions NOT LIKE '%"wsdl"%';

-- Note: Projects with NULL or empty allowed_extensions will automatically use
-- the default ALLOWED_EXTENSIONS list from main.rs, which now includes 'xsd' and 'wsdl'
