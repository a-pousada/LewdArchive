import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { LoaderCircle, Pencil, Plus, Send, Trash2, Volume2, Check } from 'lucide-react';
import { communityApi } from './communityApi';
import type { Webhook } from '../api';
import { Modal, Toggle } from '../components';
import { useCategories } from './Categories';

function webhookCategories(webhook?: Webhook): string[] | null {
  if (!webhook) return null;
  if (webhook.category_filters && webhook.category_filters.length > 0) {
    return webhook.category_filters;
  }
  if (webhook.category_filter != null) {
    return [webhook.category_filter];
  }
  return null;
}

function parseTagInput(raw: string): string[] {
  const tags: string[] = [];
  for (const part of raw.split(',')) {
    const clean = part.trim().replace(/^#+/, '').toLowerCase().replace(/\s+/g, '_');
    if (clean && !tags.includes(clean)) tags.push(clean);
  }
  return tags;
}

function formatCategorySummary(categories: string[] | null): string {
  if (!categories || categories.length === 0) return 'All categories';
  return categories.map((c) => c || 'Uncategorized').join(', ');
}

export default function WebhookManager({ serverId }: { serverId: string }) {
  const base = `/servers/${serverId}/webhooks`;
  const api = communityApi;
  const client = useQueryClient();
  const query = useQuery({
    queryKey: ['webhooks', serverId],
    queryFn: () => api<Webhook[]>(base),
  });
  const [editing, setEditing] = useState<Webhook | 'new' | null>(null);
  const [removing, setRemoving] = useState<Webhook | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testStatus, setTestStatus] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  async function sendTest(webhook: Webhook) {
    setTestingId(webhook.id);
    setTestStatus('');
    setError('');
    try {
      await api(`${base}/${webhook.id}/test`, 'POST');
      setTestStatus(`Test message sent to ${webhook.name}.`);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setTestingId(null);
    }
  }
  async function remove() {
    if (!removing) return;
    setBusy(true);
    setError('');
    try {
      await api(`${base}/${removing.id}`, 'DELETE');
      setRemoving(null);
      await client.invalidateQueries();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="settings-card webhook-manager">
      <div className="settings-section-head">
        <span className="stat-icon discord-icon">
          <Volume2 size={21} />
        </span>
        <div>
          <h2>Webhooks</h2>
          <p>Destinations for this server’s artists only.</p>
        </div>
      </div>
      <p className="hint">
        New posts go to enabled webhooks matching their artist categories and optional tag filters.
        Each destination has its own delivery status and retries.
      </p>
      {testStatus && (
        <p className="hint" role="status">
          {testStatus}
        </p>
      )}
      {error && !removing && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {query.isPending && <p className="hint">Loading webhooks...</p>}
      {query.isError && (
        <p className="error" role="alert">
          {query.error.message}{' '}
          <button type="button" onClick={() => void query.refetch()}>
            Try again
          </button>
        </p>
      )}
      <div className="webhook-list">
        {query.data?.map((webhook) => (
          <article className="webhook-row" key={webhook.id}>
            <img
              src={webhook.avatar_url || '/favicon.svg'}
              alt=""
              referrerPolicy="no-referrer"
              onError={(e) => {
                e.currentTarget.onerror = null;
                e.currentTarget.src = '/favicon.svg';
              }}
            />
            <div className="webhook-info">
              <strong>{webhook.name}</strong>
              <small>{webhook.username || 'Default webhook identity'}</small>
              <small>{formatCategorySummary(webhookCategories(webhook))}</small>
              {(webhook.include_tags?.length ?? 0) > 0 && (
                <small>Include: {webhook.include_tags!.map((t) => `#${t}`).join(', ')}</small>
              )}
              {(webhook.exclude_tags?.length ?? 0) > 0 && (
                <small>Exclude: {webhook.exclude_tags!.map((t) => `#${t}`).join(', ')}</small>
              )}
              <span className={`badge ${webhook.enabled ? 'good' : ''}`}>
                {webhook.enabled ? 'Enabled' : 'Paused'}
              </span>
            </div>
            <button
              type="button"
              className="icon-button"
              aria-label={`Test webhook ${webhook.name}`}
              title="Send test message"
              disabled={testingId !== null}
              onClick={() => void sendTest(webhook)}
            >
              {testingId === webhook.id ? (
                <LoaderCircle className="spin" size={16} />
              ) : (
                <Send size={16} />
              )}
            </button>
            <button
              type="button"
              className="icon-button"
              aria-label={`Edit webhook ${webhook.name}`}
              onClick={() => setEditing(webhook)}
            >
              <Pencil size={16} />
            </button>
            <button
              type="button"
              className="icon-button danger-text"
              aria-label={`Delete webhook ${webhook.name}`}
              onClick={() => {
                setError('');
                setRemoving(webhook);
              }}
            >
              <Trash2 size={16} />
            </button>
          </article>
        ))}
      </div>
      {query.data?.length === 0 && (
        <div className="webhook-empty">
          <Volume2 size={26} />
          <strong>Your first webhook starts here.</strong>
          <p>Add a webhook (Discord, Fluxer, or compatible) to receive new artwork.</p>
        </div>
      )}
      <button type="button" className="button secondary" onClick={() => setEditing('new')}>
        <Plus size={16} />
        Add webhook
      </button>
      {editing && (
        <WebhookEditor
          base={base}
          api={api}
          webhook={editing === 'new' ? undefined : editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            void client.invalidateQueries();
          }}
        />
      )}
      {removing && (
        <Modal
          title="Remove this webhook?"
          subtitle={removing.name}
          onClose={() => !busy && setRemoving(null)}
        >
          <p className="modal-copy">
            This removes the destination and its queued deliveries and activity records from
            LewdArchive. Other destinations are unaffected. Messages already sent to destinations
            remain there.
          </p>
          {error && (
            <p className="error" role="alert">
              {error}
            </p>
          )}
          <div className="modal-actions">
            <button
              type="button"
              className="button secondary"
              disabled={busy}
              onClick={() => setRemoving(null)}
            >
              Cancel
            </button>
            <button
              type="button"
              className="button destructive"
              disabled={busy}
              onClick={() => void remove()}
            >
              {busy ? <LoaderCircle className="spin" size={16} /> : <Trash2 size={16} />}Remove
              webhook
            </button>
          </div>
        </Modal>
      )}
    </section>
  );
}
function WebhookEditor({
  base,
  api,
  webhook,
  onClose,
  onSaved,
}: {
  base: string;
  api: typeof communityApi;
  webhook?: Webhook;
  onClose: () => void;
  onSaved: () => void;
}) {
  const categories = useCategories(base.split('/')[2]);
  const [selectedCategories, setSelectedCategories] = useState<string[] | null>(() =>
    webhookCategories(webhook),
  );
  const [includeTags, setIncludeTags] = useState(() => (webhook?.include_tags ?? []).join(', '));
  const [excludeTags, setExcludeTags] = useState(() => (webhook?.exclude_tags ?? []).join(', '));
  const [name, setName] = useState(webhook?.name || '');
  const [url, setUrl] = useState('');
  const [username, setUsername] = useState(webhook?.username ?? 'LewdArchive');
  const [avatar, setAvatar] = useState(webhook?.avatar_url || '');
  const [color, setColor] = useState(
    `#${(webhook?.embed_color ?? 14978709).toString(16).padStart(6, '0')}`,
  );
  const [enabled, setEnabled] = useState(webhook?.enabled ?? true);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  const availableCategories = [
    ...new Set([
      ...(Array.isArray(categories.data) ? categories.data.map((r) => r.category) : []),
      ...(selectedCategories ?? []).filter((c) => c !== ''),
    ]),
  ];

  function toggleCategory(value: string) {
    setSelectedCategories((current) => {
      if (!current) return [value];
      if (current.includes(value)) {
        const next = current.filter((c) => c !== value);
        return next.length > 0 ? next : null;
      }
      return [...current, value];
    });
  }

  return (
    <Modal
      wide
      title={webhook ? 'Edit webhook' : 'Add webhook'}
      subtitle="Give this destination its own identity."
      onClose={() => !busy && onClose()}
    >
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          setError('');
          try {
            const categoryList =
              selectedCategories && selectedCategories.length > 0 ? selectedCategories : null;
            await api(webhook ? `${base}/${webhook.id}` : base, webhook ? 'PUT' : 'POST', {
              name,
              ...(url.trim() ? { url: url.trim() } : {}),
              username,
              avatar_url: avatar,
              embed_color: parseInt(color.slice(1), 16),
              category_filter: categoryList && categoryList.length === 1 ? categoryList[0] : null,
              category_filters: categoryList,
              include_tags: parseTagInput(includeTags),
              exclude_tags: parseTagInput(excludeTags),
              enabled,
            });
            onSaved();
          } catch (e) {
            setError((e as Error).message);
          } finally {
            setBusy(false);
          }
        }}
      >
        <div className="webhook-editor-grid">
          <div className="form-grid">
            <label className="full">
              Destination name
              <input
                autoFocus
                aria-label="Destination name"
                aria-describedby="webhook-name-help"
                required
                maxLength={120}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. Artist updates"
              />
              <small id="webhook-name-help">Only shown in your archive and Activity.</small>
            </label>
            <label className="full">
              Webhook URL
              <input
                type="password"
                aria-label="Webhook URL"
                aria-describedby="webhook-url-help"
                required={!webhook}
                autoComplete="new-password"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder={
                  webhook
                    ? 'Leave blank to keep the saved URL'
                    : 'https://discord.com/api/webhooks/... or https://fluxer.app/...'
                }
              />
              <small id="webhook-url-help">
                Discord, Fluxer, or any service supporting Discord-compatible webhooks.
              </small>
            </label>
            <label className="full">
              Display name
              <input
                maxLength={80}
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="Use the webhook’s default name"
              />
            </label>
            <label className="full">
              Avatar URL
              <input
                type="url"
                aria-label="Avatar URL"
                aria-describedby="webhook-avatar-help"
                maxLength={4096}
                value={avatar}
                onChange={(e) => setAvatar(e.target.value)}
                placeholder="https://example.com/avatar.png"
              />
              <small id="webhook-avatar-help">
                Public HTTPS image. Leave blank to use the destination’s default avatar.
              </small>
            </label>
            <div className="full">
              <label>
                Category filter
                <select
                  aria-label="Category filter"
                  value={
                    selectedCategories == null
                      ? '*'
                      : selectedCategories.length === 1
                        ? selectedCategories[0]
                        : '__multi__'
                  }
                  onChange={(e) => {
                    if (e.target.value === '*') setSelectedCategories(null);
                    else if (e.target.value !== '__multi__')
                      setSelectedCategories([e.target.value]);
                  }}
                >
                  <option value="*">All categories</option>
                  <option value="">Uncategorized only</option>
                  {availableCategories.map((c) => (
                    <option key={c} value={c}>
                      {c}
                    </option>
                  ))}
                  {selectedCategories && selectedCategories.length > 1 && (
                    <option value="__multi__">
                      Multiple categories ({selectedCategories.length})
                    </option>
                  )}
                </select>
              </label>
              <div className="webhook-category-chips" aria-label="Combine categories">
                <button
                  type="button"
                  className={`category-chip ${selectedCategories == null ? 'active' : ''}`}
                  onClick={() => setSelectedCategories(null)}
                >
                  All categories
                </button>
                <button
                  type="button"
                  className={`category-chip ${selectedCategories?.includes('') ? 'active' : ''}`}
                  onClick={() => toggleCategory('')}
                >
                  Uncategorized
                </button>
                {availableCategories.map((c) => (
                  <button
                    key={c}
                    type="button"
                    className={`category-chip ${selectedCategories?.includes(c) ? 'active' : ''}`}
                    onClick={() => toggleCategory(c)}
                  >
                    {c}
                  </button>
                ))}
              </div>
              <small>
                Only posts from artists in the selected categories go to this destination.
              </small>
            </div>
            <label className="full">
              Include tags
              <input
                aria-label="Include tags"
                value={includeTags}
                onChange={(e) => setIncludeTags(e.target.value)}
                placeholder="e.g. 3d, blender, photorealistic"
              />
              <small>Optional comma-separated tags. At least one must match.</small>
            </label>
            <label className="full">
              Exclude tags
              <input
                aria-label="Exclude tags"
                value={excludeTags}
                onChange={(e) => setExcludeTags(e.target.value)}
                placeholder="e.g. furry, monochrome"
              />
              <small>Optional comma-separated tags. Matching posts are skipped.</small>
            </label>
            <label className="full color-field">
              Message color
              <input type="color" value={color} onChange={(e) => setColor(e.target.value)} />
            </label>
          </div>
          <aside className="webhook-preview">
            <span className="eyebrow">MESSAGE PREVIEW</span>
            <div className="discord-preview">
              <img
                key={avatar}
                src={avatar.startsWith('https://') ? avatar : '/favicon.svg'}
                alt="Avatar preview"
                referrerPolicy="no-referrer"
                onError={(e) => {
                  e.currentTarget.onerror = null;
                  e.currentTarget.src = '/favicon.svg';
                }}
              />
              <div>
                <strong>
                  {username || 'Webhook name'} <span>APP</span>
                </strong>
                <div className="discord-embed" style={{ borderLeftColor: color }}>
                  <small>ARTIST NAME</small>
                  <h4>A new discovery</h4>
                  <p>Images, a description, and a link to the original post.</p>
                  <div className="preview-image">
                    <Volume2 size={32} />
                  </div>
                  <small>LewdArchive · Your feed</small>
                </div>
              </div>
            </div>
            <p className="hint">
              Name, avatar, and color apply to messages sent by LewdArchive. Blank identity fields
              keep the destination’s defaults.
            </p>
          </aside>
        </div>
        <Toggle
          checked={enabled}
          onChange={setEnabled}
          label="Enable this webhook"
          description="Pausing keeps existing deliveries queued. New posts go only to enabled destinations."
        />
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        <div className="modal-actions">
          <button className="button secondary" type="button" disabled={busy} onClick={onClose}>
            Cancel
          </button>
          <button className="button primary" disabled={busy}>
            {busy ? <LoaderCircle className="spin" size={16} /> : <Check size={16} />}Save webhook
          </button>
        </div>
      </form>
    </Modal>
  );
}
