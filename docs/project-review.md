# Project review — October 2026

This review builds on the existing Rust/Axum, PostgreSQL and React implementation. It is a focused source and integration review, not a production security audit. Existing uncommitted community and administration work is preserved.

## Delivered in this iteration

- Exposed the existing server-owned artist category in creation and profile editing, and connected it to channel routing and library filters.
- Added category filters to compatible webhooks, including manual replay. The former fanout sent every post to every enabled server webhook.
- Added server-owned tags, manual editing, reviewed AI suggestions and opt-in automatic image classification through a persistent queue. Manual changes win over late inference results.
- Replaced LLM tagging with local WD SwinV2 v3 on CPU, pinned weights, configurable thresholds and threads, and measured inference timings.
- Added richer notification metadata and bounded its text budget so larger categories and tag sets do not exceed embed limits.
- Added category badges, tag chips, a channel mapping panel, suggestion review, loading/error states and responsive filters.
- Corrected HTTP 404 for `/community/`: the SPA previously displayed correctly but returned an error status with a trailing slash.
- Added regression checks for isolation, filters, queues, migration preservation and desktop/mobile workflows.

## Suggested next iterations

1. **AI job activity and model evaluation.** Expose tagging job status, errors and explicit retries in Delivery activity. Evaluate an operator-chosen model against a small, representative, reviewed dataset before enabling it broadly. The current tests verify integration behavior, not classification accuracy.
2. **Artist search and pagination.** The artist endpoint currently returns the entire server library. Add paginated search and category filtering in SQL for larger collections; the current artist category filter is in the browser.
3. **Retention and disk usage.** Downloaded files survive removal of their feed metadata. Add a usage report and a preview of orphaned files before any cleanup policy. Preserve backups and avoid automatic deletion by default.
4. **Operational visibility.** Add feed freshness, queue age and per-destination success metrics. AI job errors are currently retained in `community_tag_jobs`, but do not yet have a dedicated activity panel.
5. **Model evaluation by medium.** WD is optimized for illustrated images. Build a reviewed evaluation set for 2D, 3D and photography before selecting specialist classifiers or changing thresholds. Measure false positives, character recognition, CPU latency and memory.
6. **Queue throughput.** The main polling/download worker is serial. Large downloads can delay subsequent feed polls. Separate download execution into a bounded worker pool, retaining PostgreSQL reservation and retry semantics.
7. **Taxonomy tools.** Add bulk artist categorization, category rename/merge previews, and tag aliases. Preserve deliberate server-specific classification rather than globally rewriting a shared feed.

## Refined implementation brief

> Extend the server-owned artist library with editable categories and post tags. Support a per-category Discord channel with fallback to the server default and independent category filters on compatible webhooks. Use local WD image classification on CPU for optional tag previews, human review and opt-in automatic tagging of newly discovered posts. Bound image sizes, concurrency and CPU threads; pin model artifacts and preserve manual edits. Persist queues and metadata per server, preserve initial silent imports, and keep routing under administrator control. Display metadata in a responsive library and bounded notification embeds. Validate authorization, shared-feed isolation, retries, inference failures and existing-data migration with mocks and a disposable database. Document setup, measured CPU performance and classification limitations.
