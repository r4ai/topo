-- Every column is nullable or has a default, so the Worker deployed before
-- this migration keeps working: it writes rows without naming these columns.
ALTER TABLE nodes ADD COLUMN priority TEXT       -- NULL: no priority
  CHECK (priority IN ('low', 'medium', 'high', 'urgent'));
ALTER TABLE nodes ADD COLUMN assignee TEXT;      -- who works on the node, or NULL
ALTER TABLE nodes ADD COLUMN prs TEXT NOT NULL DEFAULT '[]';
                                                 -- JSON array of pull request URLs
ALTER TABLE nodes ADD COLUMN created_at TEXT;    -- RFC 3339 in UTC, or NULL when unknown
ALTER TABLE nodes ADD COLUMN updated_at TEXT;    -- RFC 3339 in UTC, or NULL when unknown
ALTER TABLE nodes ADD COLUMN completed_at TEXT;  -- RFC 3339 in UTC; set only while status is 'done'
