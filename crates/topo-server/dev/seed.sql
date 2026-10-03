-- Local development only: a user and the token `topo_dev` (stored as its SHA-256).
-- Apply with: wrangler d1 execute topo --local --file dev/seed.sql
INSERT INTO users (id, login) VALUES (1, 'dev') ON CONFLICT (id) DO NOTHING;
INSERT INTO tokens (id, hash, user_id, name) VALUES ('devtoken', '093e3c00a9f4c688655012f30eb0907604dafa2e152b1bd8c42137a1538d4cd8', 1, 'dev')
ON CONFLICT (id) DO NOTHING;
