import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Archive, LoaderCircle, Plus } from 'lucide-react';
import { api, sources } from '../api';
import { buildFeedUrl, feedTemplates } from '../feedTemplates';
import type { FeedTemplate, PawchiveService } from '../feedTemplates';
import type { Feed } from '../api';
import { Modal, Toggle } from '../components';
export default function FeedForm({
  feed,
  onClose,
  onSubmit,
}: {
  feed?: Feed;
  onClose: () => void;
  onSubmit: (data: unknown) => Promise<boolean>;
}) {
  const [form, setForm] = useState({
    name: feed?.name || '',
    url: feed?.url || '',
    source: feed?.source || 'rss',
    poll_minutes: feed?.poll_minutes || 15,
    enabled: feed?.enabled ?? true,
    notify: feed?.notify ?? true,
    auto_download: feed?.auto_download ?? false,
  });
  const [template, setTemplate] = useState<FeedTemplate>(feed ? 'custom' : 'pawchive');
  const [identifier, setIdentifier] = useState('');
  const [service, setService] = useState<PawchiveService>('fanbox');
  const [baseOverride, setBaseOverride] = useState<string | null>(null);
  const options = useQuery({
    queryKey: ['feed-options'],
    queryFn: () => api<{ rsshub_base_url: string }>('/feed-options'),
    enabled: !feed && template !== 'custom',
  });
  const baseUrl = baseOverride ?? options.data?.rsshub_base_url ?? '';
  const automatic = !feed && template !== 'custom';
  let generatedUrl = '';
  let urlError = '';
  if (automatic) {
    try {
      generatedUrl = buildFeedUrl(template, baseUrl, identifier, service);
    } catch (error) {
      urlError = (error as Error).message;
    }
  }
  const [busy, setBusy] = useState(false);
  const update = (key: string, value: unknown) => setForm((f) => ({ ...f, [key]: value }));
  return (
    <Modal
      title={feed ? 'Edit connection' : 'Follow your next discovery.'}
      subtitle="Choose a service or paste a custom feed URL."
      onClose={onClose}
    >
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          if (automatic && !generatedUrl) return;
          setBusy(true);
          try {
            await onSubmit(automatic ? { ...form, url: generatedUrl, source: template } : form);
          } finally {
            setBusy(false);
          }
        }}
      >
        <div className="form-grid">
          <label className="full">
            Feed name
            <input
              autoFocus
              required
              maxLength={120}
              placeholder="e.g. Aoi’s sketchbook"
              value={form.name}
              onChange={(e) => update('name', e.target.value)}
            />
          </label>
          {!feed && (
            <label className="full">
              Service
              <select
                aria-label="Service"
                value={template}
                onChange={(e) => {
                  setTemplate(e.target.value as FeedTemplate);
                  setIdentifier('');
                }}
              >
                {feedTemplates.map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.label}
                  </option>
                ))}
              </select>
            </label>
          )}
          {automatic ? (
            <>
              {template === 'pawchive' && (
                <label>
                  Pawchive platform
                  <select
                    aria-label="Pawchive platform"
                    value={service}
                    onChange={(e) => setService(e.target.value as PawchiveService)}
                  >
                    <option value="fanbox">Fanbox</option>
                    <option value="patreon">Patreon</option>
                  </select>
                </label>
              )}
              <label className={template === 'pawchive' ? '' : 'full'}>
                {template === 'bluesky' ? 'Artist handle' : 'Artist ID'}
                <input
                  required
                  maxLength={253}
                  aria-label={template === 'bluesky' ? 'Artist handle' : 'Artist ID'}
                  aria-describedby="artist-id-help"
                  inputMode={template === 'bluesky' ? 'text' : 'numeric'}
                  placeholder={
                    template === 'bluesky'
                      ? 'artist.bsky.social'
                      : template === 'pawchive'
                        ? '22445'
                        : '15288095'
                  }
                  value={identifier}
                  onChange={(e) => setIdentifier(e.target.value)}
                />
                <small id="artist-id-help">
                  {template === 'bluesky'
                    ? 'Use the full handle, including the domain.'
                    : 'Copy the number from the artist’s profile URL.'}
                </small>
              </label>
              <label className="full">
                RSSHub instance
                <input
                  type="url"
                  required
                  aria-label="RSSHub instance"
                  aria-describedby="rsshub-instance-help"
                  placeholder={
                    options.isPending ? 'Loading the configured instance...' : 'https://rsshub.app'
                  }
                  value={baseUrl}
                  onChange={(e) => setBaseOverride(e.target.value)}
                />
                <small id="rsshub-instance-help">
                  Your configured instance is filled in. You can use a different one for this feed.
                </small>
              </label>
              {options.isError && baseOverride === null && (
                <div className="notice full" role="alert">
                  Unable to load the instance. Enter the address above or
                  <button type="button" onClick={() => void options.refetch()}>
                    try again
                  </button>
                  .
                </div>
              )}
              <div className="feed-url-preview full" aria-live="polite">
                <span>Generated feed URL</span>
                {generatedUrl ? (
                  <code>{generatedUrl}</code>
                ) : (
                  <p>Enter the artist details to generate the URL.</p>
                )}
                {!!identifier.trim() && !!baseUrl && !!urlError && (
                  <p className="error">{urlError}</p>
                )}
              </div>
              {template === 'pixiv' && (
                <p className="hint full">
                  RSSHub needs Pixiv credentials configured to fetch this feed.
                </p>
              )}
            </>
          ) : (
            <>
              <label className="full">
                Feed URL
                <input
                  type="url"
                  aria-label="Feed URL"
                  aria-describedby="feed-url-help"
                  required
                  readOnly={!!feed}
                  placeholder="https://example.com/feed.xml"
                  value={form.url}
                  onChange={(e) => update('url', e.target.value)}
                />
                <small id="feed-url-help">
                  {feed
                    ? 'The URL identifies this connection. Create a new feed to use a different URL.'
                    : 'Paste the full URL: RSS, Atom, Xcancel, or any RSSHub route.'}
                </small>
              </label>
              <label>
                Source
                <select
                  aria-label="Source"
                  value={form.source}
                  onChange={(e) => update('source', e.target.value)}
                >
                  {sources.map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.label}
                    </option>
                  ))}
                </select>
              </label>
            </>
          )}
          <label>
            Check every
            <select
              value={form.poll_minutes}
              onChange={(e) => update('poll_minutes', Number(e.target.value))}
            >
              {[5, 10, 15, 30, 60, 120, 360, 720, 1440].map((m) => (
                <option key={m} value={m}>
                  {m < 60 ? `${m} minutes` : `${m / 60} hour(s)`}
                </option>
              ))}
            </select>
          </label>
        </div>
        <div className="form-options">
          <Toggle
            checked={form.notify}
            onChange={(v) => update('notify', v)}
            label="Notify on Discord"
            description="Send new posts to your configured webhook."
          />
          <Toggle
            checked={form.auto_download}
            onChange={(v) => update('auto_download', v)}
            label="Download files automatically"
            description="Save images and videos from upcoming posts."
          />
          <Toggle
            checked={form.enabled}
            onChange={(v) => update('enabled', v)}
            label="Follow this feed"
            description="Keep scheduled checks enabled."
          />
        </div>
        {!feed && (
          <div className="notice">
            <Archive size={17} />
            <span>
              The first sync imports history without sending notifications or downloading files.
              Automation starts with new posts.
            </span>
          </div>
        )}
        <div className="modal-actions">
          <button type="button" className="button secondary" onClick={onClose}>
            Cancel
          </button>
          <button className="button primary" disabled={busy || (automatic && !generatedUrl)}>
            {busy ? <LoaderCircle className="spin" size={16} /> : <Plus size={16} />}{' '}
            {feed ? 'Save changes' : 'Add feed'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
