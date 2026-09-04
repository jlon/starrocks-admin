-- Add account lock fields to users table
ALTER TABLE users ADD COLUMN failed_login_attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN locked_until TIMESTAMP;

-- Create index for locked users
CREATE INDEX IF NOT EXISTS idx_users_locked_until ON users(locked_until);