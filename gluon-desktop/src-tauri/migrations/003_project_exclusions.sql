-- Dodaj kolumnę do przechowywania wykluczonych ścieżek
ALTER TABLE projects ADD COLUMN excluded_paths TEXT DEFAULT '[]';
-- Format: JSON array stringów, np: '["node_modules", ".git", "custom_folder"]'