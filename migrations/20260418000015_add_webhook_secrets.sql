ALTER TABLE agents ADD COLUMN webhook_secret TEXT;

-- Auto-generate secrets for existing agents
UPDATE agents SET webhook_secret = encode(gen_random_bytes(32), 'hex') WHERE webhook_secret IS NULL;

ALTER TABLE agents ALTER COLUMN webhook_secret SET NOT NULL;
ALTER TABLE agents ALTER COLUMN webhook_secret SET DEFAULT encode(gen_random_bytes(32), 'hex');
