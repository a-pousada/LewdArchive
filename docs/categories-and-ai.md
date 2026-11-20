# Categories, routing and AI tags

Artists have one category per server. Posts inherit their artist's category and have a separate, server-owned list of tags. Two servers following the same RSS feed can use different categories, tags and destinations.

## Library and routing

Choose a category when tracking an artist, or use **Artists → Edit profile**. Suggested labels are **2D Illustration**, **3D Art**, **Photography** and **Mixed Media**. Custom labels, including your existing OPML categories, are supported. An empty category means Uncategorized. Labels are case-sensitive and trimmed; use the suggestions to keep spelling consistent.

**Server settings → Category channels** maps an artist category to a Discord bot channel. Select a row to edit its destination, or enter a custom category. The application verifies that the channel is in the same Discord server and readable by the bot. The bot also needs Send Messages and Embed Links for delivery.

The category channel takes priority over the default announcement channel. Without an override, the default channel is used. If neither exists, no bot delivery is queued. A blank category channel removes that override. Channel changes affect future jobs; existing bot jobs retain their original channel, including retries.

Each webhook has an independent **Category filter**: all categories, uncategorized only, or one category. Automatic and manual webhook sends both respect it. Multiple matching destinations each receive a delivery. Existing webhooks default to all categories. Changing a filter can leave older, nonmatching queued jobs paused until they match again; it never moves them to a different webhook. Bot routing and webhooks are independent: enabling both can intentionally publish twice to the same destination.

The library supports category and exact tag filters. Artist lists also support category filtering. Categories remain part of OPML exports; post tags and destination configuration do not.

## Manual and reviewed AI tagging

Open a post and expand **Edit tags and AI suggestions**. Managers can edit up to 30 comma-separated tags, each at most 48 characters. Tags normalize to lowercase with underscores, remove duplicates, and reject markup and mention syntax.

**Suggest with AI** creates a preview. Remove unwanted suggestions, choose **Add to draft**, then **Save tags**. Generating a preview never changes saved tags or the artist category. Manual editing works when AI is disabled or unavailable.

The tagger is **WD SwinV2 v3**, running locally on CPU through ONNX Runtime. It is an image classifier with a fixed vocabulary, not an LLM: there is no prompt, text captioning or Ollama dependency. See the [model comparison and measured CPU timings](wd-tagger.md).

Set these values in `.env` to use the bundled service:

```dotenv
WD_TAGGER_URL=http://tagger:5000
WD_CPU_THREADS=2
WD_GENERAL_THRESHOLD=0.35
WD_CHARACTER_THRESHOLD=0.85
```

Start it with Docker Compose 2.20+:

```sh
docker compose --profile ai up -d --build app tagger
docker compose --profile ai logs -f tagger
```

The first start downloads about 467 MB of model weights plus the vocabulary from Hugging Face. Both artifacts use an immutable revision and verified SHA-256 checksums. They persist in the `wd_models` volume. Startup reports readiness only after loading the model; Compose waits for a healthy tagger before starting the app when the `ai` profile is active. Subsequent starts use the cache. No images are sent to Hugging Face. The service does not persist submitted images, and ONNX telemetry is disabled.

An empty `WD_TAGGER_URL` disables inference. The URL must be reachable from the application: `localhost` inside a container refers to that container. The bundled service exposes no host port and is intended only for the trusted application network; do not publish it publicly. For native development, run `pip install -r tagger/requirements.txt`, then `MODEL_CACHE=.test-runtime/wd-cache python tagger/service.py` and set the backend URL to `http://127.0.0.1:5000`. Native execution binds to loopback by default; the Docker image sets `WD_BIND_ADDRESS=0.0.0.0` for container networking.

Preview and automatic tagging analyze the first two images in a post. The backend fetches public JPEG, PNG or WebP files, at most 5 MiB each, and sends image bytes to the configured service. Private hosts and unsafe redirects are blocked, even when the feed uses a private RSSHub. The decoder accepts at most 4096 pixels per side and 16 megapixels. Animated images use the first frame; videos are not analyzed. Posts without images have no AI preview and automatic jobs finish without retries. Titles, descriptions, artist identities and category labels are not inference inputs.

The service uses one inference at a time, two CPU threads by default, and configurable general/character thresholds. Higher thresholds produce fewer tags. Scores from two images merge by maximum confidence, duplicates are removed, and the top 30 tags are retained. A failed image does not discard successful results from the other image; if both fail, normal retries apply. Ratings are omitted and never control routing. Tags retain parentheses in names such as `character_(series)` while rejecting mentions and markup. Requests are bounded by time, input size and response size. Endpoint configuration is operator-only.

WD was trained primarily on illustrated images. Review results for photography and 3D content; current classification quality on this library has not been measured. Its training cutoff also limits recognition of newer characters. The model is not a safety, age or consent detector.

For installations migrating from the earlier implementation, replace `AI_OLLAMA_URL`, `AI_MODEL` and `AI_VISION` with the settings above. Existing manual and generated tags remain saved. Existing server opt-ins remain enabled, and pending jobs switch to image analysis once WD is configured. Remove obsolete Ollama settings; no history is automatically retagged.

## Automatic tags

After the operator configures a model, a server manager may enable **Automatically tag new posts** in Server settings. It is off by default and applies only to newly discovered posts after a feed's initial import. Historical posts remain silent and can use manual previews.

Automatic classification analyzes images through the same local service as previews. It runs in a persistent queue separate from polling and downloads, with up to three attempts and stale-job recovery. Notifications wait for tags while a job is active, for at most five minutes from discovery. After failure, timeout of this waiting window, disabling automatic tags, or disabling the provider, normal delivery can proceed without tags. A later AI result does not edit already-sent Discord messages.

Automatic tags are marked as AI-generated in the post view. Managers can correct or clear them. A manual save, including an empty list, always takes precedence over an in-flight AI result. Existing saved tags are never overwritten by automatic inference. Category/channel selection is always controlled by the manager, never by the model.

Embeds include artist, gallery size and source, plus the destination server's category and saved tags. Previewed but unsaved suggestions are not published. Content limits keep metadata within Discord's embed budget. Mentions remain governed by existing user opt-ins.

## Validation

Rust tests use a local mock WD endpoint and fixed image bytes; production image fetching always enforces public-host restrictions. Python tests cover preprocessing, thresholds, checksums, malformed/oversized images and the HTTP contract. The opt-in `WD_MODEL_SMOKE=1` test loads the real pinned model and classifies a synthetic non-sensitive image through HTTP. PostgreSQL tests cover tenant isolation, category routing, webhook matching, filters, queue delivery gating, generated tags, manual precedence, text-only posts and provider failure. Browser tests cover review, saving, missing images, provider errors and routing on desktop/mobile. No tests send messages to Discord.
