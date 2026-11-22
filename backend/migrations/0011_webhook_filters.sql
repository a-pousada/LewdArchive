-- Multi-category and tag-based filtering for webhooks, plus skipped delivery status.
ALTER TABLE webhooks
    ADD COLUMN category_filters text[] CHECK (
        category_filters IS NULL
        OR (cardinality(category_filters) BETWEEN 1 AND 50)
    ),
    ADD COLUMN include_tags text[] NOT NULL DEFAULT '{}' CHECK (cardinality(include_tags) <= 30),
    ADD COLUMN exclude_tags text[] NOT NULL DEFAULT '{}' CHECK (cardinality(exclude_tags) <= 30);

UPDATE webhooks
SET category_filters = ARRAY[category_filter]
WHERE category_filter IS NOT NULL AND category_filters IS NULL;

ALTER TABLE jobs DROP CONSTRAINT IF EXISTS jobs_status_check;
ALTER TABLE jobs ADD CONSTRAINT jobs_status_check
    CHECK (status IN ('pending', 'processing', 'done', 'failed', 'skipped'));
