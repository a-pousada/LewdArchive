# Feed transfer

Server administrators can import and export OPML from the Artists page in Community. Miniflux categories and disabled flags are supported. Preview identifies invalid URLs and duplicates within the selected server. The same feed can belong to multiple servers without becoming a duplicate across their collections.

Imports accept up to 1,000 feeds and 2 MiB of XML. DTDs and external entities are rejected. Initial feed history is silent. Announcement preferences apply to the importing server. Shared feeds keep their existing polling configuration.

Endpoints and request fields are documented in [HTTP API](api.md). Installation-level feed transfer endpoints have been removed.
