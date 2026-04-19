-- Ensure source column is NOT NULL with empty string default
UPDATE events SET source = '' WHERE source IS NULL;
ALTER TABLE events ALTER COLUMN source SET NOT NULL;
ALTER TABLE events ALTER COLUMN source SET DEFAULT '';
