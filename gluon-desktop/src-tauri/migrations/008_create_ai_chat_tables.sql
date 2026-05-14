-- ============================================================================
-- AI CHAT SYSTEM - Database Schema
-- Migration 008: Create tables for AI Chat functionality
-- ============================================================================

-- AI Providers table (Gemini, Claude, GPT-4, VS Code, etc.)
CREATE TABLE IF NOT EXISTS ai_providers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,           -- "Gemini", "Claude", "GPT-4"
    provider_type TEXT NOT NULL,         -- "gemini", "claude", "openai", "vscode"
    api_endpoint TEXT,                   -- Base URL for API
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Chat Sessions table (individual conversations)
CREATE TABLE IF NOT EXISTS chat_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id INTEGER NOT NULL,
    title TEXT NOT NULL,                 -- Auto-generated or user-defined
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    is_pinned BOOLEAN DEFAULT 0,         -- Pin favorite sessions
    token_usage_total INTEGER DEFAULT 0, -- Token tracking
    FOREIGN KEY (provider_id) REFERENCES ai_providers(id) ON DELETE CASCADE
);

-- Chat Messages table (messages within sessions)
CREATE TABLE IF NOT EXISTS chat_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,               -- Message content (markdown)
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    token_count INTEGER DEFAULT 0,       -- Token count for this message
    FOREIGN KEY (session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE
);

-- API Keys metadata table (actual keys stored in system keyring)
CREATE TABLE IF NOT EXISTS ai_api_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id INTEGER NOT NULL UNIQUE,
    key_name TEXT NOT NULL,              -- "gemini_api_key", "claude_api_key"
    is_configured BOOLEAN DEFAULT 0,     -- Whether key is set
    last_verified TEXT,                  -- Timestamp of last successful use
    FOREIGN KEY (provider_id) REFERENCES ai_providers(id) ON DELETE CASCADE
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_chat_sessions_provider ON chat_sessions(provider_id);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_updated ON chat_sessions(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id);
CREATE INDEX IF NOT EXISTS idx_chat_messages_created ON chat_messages(created_at ASC);

-- Seed default AI providers
INSERT INTO ai_providers (name, provider_type, api_endpoint) VALUES
('Gemini', 'gemini', 'https://generativelanguage.googleapis.com/v1beta'),
('Claude', 'claude', 'https://api.anthropic.com/v1'),
('GPT-4', 'openai', 'https://api.openai.com/v1'),
('VS Code', 'vscode', NULL);

-- ============================================================================
-- Notes:
-- - API keys are stored in system keyring (Windows Credential Manager)
-- - token_usage_total is cumulative for the entire session
-- - is_pinned allows users to favorite important conversations
-- - updated_at is updated on every new message for sorting
-- ============================================================================
