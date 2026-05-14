-- Add language column to environments table with a default value
ALTER TABLE environments ADD COLUMN language TEXT NOT NULL DEFAULT 'en';