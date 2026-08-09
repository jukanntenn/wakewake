ALTER TABLE users
    DROP COLUMN IF EXISTS verification_sent_at,
    DROP COLUMN IF EXISTS email_verified;
