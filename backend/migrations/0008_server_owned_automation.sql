-- Keep historical installation data intact, but retire its automation.
ALTER TABLE community_artists ADD COLUMN auto_download boolean NOT NULL DEFAULT false;
ALTER TABLE community_artists ADD COLUMN category text NOT NULL DEFAULT '' CHECK(char_length(category)<=500);
UPDATE community_artists a SET auto_download=f.auto_download,category=f.category FROM feeds f WHERE f.id=a.feed_id;
UPDATE webhooks SET enabled=false WHERE community_id IS NULL;
UPDATE feeds SET notify=false,auto_download=false;
UPDATE jobs SET status='failed',last_error='Installation archive automation retired',locked_at=NULL
WHERE status IN ('pending','processing') AND kind='discord' AND (webhook_id IS NULL OR webhook_id IN(SELECT id FROM webhooks WHERE community_id IS NULL));

UPDATE entries SET summary=regexp_replace(summary, '^Downloads\s*/\s*Files.*?Content\s+', '') WHERE summary ~ '^Downloads\s*/\s*Files.*Content';
UPDATE entries SET summary='' WHERE summary ~ '^Downloads\s*/\s*Files' AND summary !~ 'Content';
