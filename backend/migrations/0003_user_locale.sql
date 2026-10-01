-- Preferred language for UI, email and notifications ('en' or 'vi').
ALTER TABLE users ADD COLUMN locale TEXT NOT NULL DEFAULT 'en' CHECK (locale IN ('en', 'vi'));
