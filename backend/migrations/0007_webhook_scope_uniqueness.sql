-- Independent servers may intentionally use the same destination.
ALTER TABLE webhooks DROP CONSTRAINT webhooks_url_key;
ALTER TABLE webhooks ADD CONSTRAINT webhooks_scope_url_key UNIQUE NULLS NOT DISTINCT (community_id,url);
