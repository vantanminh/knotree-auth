-- Services (OAuth clients) carry a public profile shown on the consent screen
-- and in the account's connected services: a description, a homepage and a
-- square logo stored on the server under CLIENT_LOGO_DIR.
ALTER TABLE oauth_clients ADD COLUMN description TEXT;
ALTER TABLE oauth_clients ADD COLUMN homepage_url TEXT;
ALTER TABLE oauth_clients ADD COLUMN logo_path TEXT;
ALTER TABLE oauth_clients ADD COLUMN updated_at TIMESTAMPTZ;
ALTER TABLE oauth_clients ADD CONSTRAINT oauth_clients_description_len
    CHECK (description IS NULL OR char_length(description) <= 1000);
