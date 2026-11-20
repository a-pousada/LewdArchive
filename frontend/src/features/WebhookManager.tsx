import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { LoaderCircle, Pencil, Plus, Trash2, Volume2, Check } from 'lucide-react';
import { communityApi } from './communityApi';
import type { Webhook } from '../api';
import { Modal, Toggle } from '../components';
import { useCategories } from './Categories';

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
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
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
        New posts go to enabled webhooks matching their artist category. Each destination has its
        own delivery status and retries.
      </p>
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
              <small>
                {webhook.category_filter == null
                  ? 'All categories'
                  : webhook.category_filter || 'Uncategorized'}
              </small>
              <span className={`badge ${webhook.enabled ? 'good' : ''}`}>
                {webhook.enabled ? 'Enabled' : 'Paused'}
              </span>
            </div>
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
  const [category, setCategory] = useState(webhook?.category_filter ?? '*');
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
            await api(webhook ? `${base}/${webhook.id}` : base, webhook ? 'PUT' : 'POST', {
              name,
              ...(url.trim() ? { url: url.trim() } : {}),
              username,
              avatar_url: avatar,
              embed_color: parseInt(color.slice(1), 16),
              category_filter: category === '*' ? null : category,
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
            <label className="full">
              Category filter
              <select
                aria-label="Category filter"
                value={category}
                onChange={(e) => setCategory(e.target.value)}
              >
                <option value="*">All categories</option>
                <option value="">Uncategorized only</option>
                {[
                  ...new Set([
                    ...(Array.isArray(categories.data)
                      ? categories.data.map((r) => r.category)
                      : []),
                    ...(category && category !== '*' ? [category] : []),
                  ]),
                ].map((c) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </select>
              <small>Only posts from artists in this category go to this destination.</small>
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
