-- ============================================================================
-- GLUON DATABASE - INITIAL SETUP
-- Combined migration (replaces 001-005)
-- ============================================================================

-- Projects table
CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE
);

-- Settings table
CREATE TABLE settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT
);

-- Default settings
INSERT INTO settings (key, value) VALUES ('port', '8743');
INSERT INTO settings (key, value) VALUES ('auto_start', 'true');
INSERT INTO settings (key, value) VALUES ('log_level', 'Info');

-- Environments table
CREATE TABLE environments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    icon TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT (datetime('now')),
    is_default BOOLEAN NOT NULL DEFAULT 0
);

-- Trigger to prevent default environment deletion
CREATE TRIGGER prevent_default_env_delete
BEFORE DELETE ON environments
FOR EACH ROW
WHEN OLD.is_default = 1
BEGIN
    SELECT RAISE(ABORT, 'The default environment cannot be deleted.');
END;

-- Prompts table
CREATE TABLE prompts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    environment_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    content TEXT,
    category TEXT NOT NULL CHECK(category IN ('system', 'environment', 'custom')),
    enabled_by_default BOOLEAN NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (environment_id) REFERENCES environments(id) ON DELETE CASCADE
);

-- Project-Environment mapping
CREATE TABLE project_environment (
    project_id INTEGER NOT NULL,
    environment_id INTEGER NOT NULL,
    PRIMARY KEY (project_id),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (environment_id) REFERENCES environments(id) ON DELETE CASCADE
);

-- Seed default environments
INSERT INTO environments (id, name, icon, is_default) VALUES
(1, 'Default', '🌍', 1),
(2, 'React Developer', '⚛️', 0),
(3, 'Unity Developer', '🎮', 0);

-- Seed default prompts for Default environment
INSERT INTO prompts (environment_id, name, content, category, enabled_by_default, sort_order) VALUES
(1, 'System Prompt', 'You are a helpful general-purpose AI assistant. Provide clear, concise, and accurate information. Analyze the provided file context to answer questions.\n\n### FORMATTING RULES\n1. ALWAYS use generic markdown code blocks (text) for any code or file content.\n2. NEVER use language-specific syntax highlighting (likejavascript, ```html) as it causes UI rendering issues.\n3. Keep all code outputs as plain, copy-pasteable text blocks.', 'system', 1, 0),
(1, 'Environment Context', 'The user is working in a general development environment. Assume standard tools and practices unless specified otherwise.', 'environment', 1, 1);

-- Seed default prompts for React Developer environment
INSERT INTO prompts (environment_id, name, content, category, enabled_by_default, sort_order) VALUES
(2, 'System Prompt (React)', 'You are an expert React developer assistant. Your advice should follow modern React best practices, including Hooks, TypeScript, and efficient component design. Analyze the provided file context to give specific and relevant code examples.', 'system', 1, 0),
(2, 'Environment Context (React)', 'The user is working on a React/TypeScript project, likely using a framework like Next.js or Create React App. The context includes components, hooks, and utility files. Focus on component-based architecture.', 'environment', 1, 1);

-- Seed default prompts for Unity Developer environment
INSERT INTO prompts (environment_id, name, content, category, enabled_by_default, sort_order) VALUES
(3, 'System Prompt (Unity)', 'You are an expert Unity and C# game developer assistant. Your answers should be optimized for performance and follow common game development patterns. Use the Unity API correctly and provide code examples within the MonoBehaviour lifecycle.', 'system', 1, 0),
(3, 'Environment Context (Unity)', 'The user is working in a Unity project with C# scripts. The context contains scenes, prefabs, and script files. Pay attention to concepts like GameObjects, Components, and the Unity event system (Update, Start, etc.).', 'environment', 1, 1);