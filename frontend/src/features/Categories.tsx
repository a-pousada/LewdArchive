import { useId, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { FolderOpen, ArrowRight } from 'lucide-react';
import { communityApi as api } from './communityApi';

export const defaultCategories = ['2D Illustration', '3D Art', 'Photography', 'Mixed Media'];
export interface CategoryRoute {
  category: string;
  channel_id: string | null;
  channel_name: string;
  artists: number;
}
export function useCategories(serverId: string) {
  return useQuery({
    queryKey: ['community', serverId, 'categories'],
    queryFn: () => api<CategoryRoute[]>(`/servers/${serverId}/categories`),
    enabled: !!serverId,
  });
}
export function CategoryField({
  value,
  onChange,
  categories = defaultCategories,
  required = false,
}: {
  value: string;
  onChange: (value: string) => void;
  categories?: string[];
  required?: boolean;
}) {
  const id = useId();
  return (
    <label>
      Artist category
      <input
        aria-label="Artist category"
        required={required}
        list={id}
        maxLength={500}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="Choose or create a category"
      />
      <datalist id={id}>
        {[...new Set([...defaultCategories, ...categories])].map((c) => (
          <option key={c} value={c} />
        ))}
      </datalist>
      <small>
        {required
          ? 'Use the same category label as the artist profile.'
          : 'Used for library filters and announcement routing. Leave blank for Uncategorized.'}
      </small>
    </label>
  );
}
export default function Categories({ serverId }: { serverId: string }) {
  const query = useCategories(serverId);
  const client = useQueryClient();
  const [category, setCategory] = useState('');
  const [channel, setChannel] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [saved, setSaved] = useState(false);
  const rows = Array.isArray(query.data) ? query.data : [];
  return (
    <section className="settings-card category-settings">
      <div className="settings-section-head">
        <span className="stat-icon">
          <FolderOpen size={21} />
        </span>
        <div>
          <h2>Category channels</h2>
          <p>Give each part of your collection its own destination.</p>
        </div>
      </div>
      <p className="hint">
        The category channel takes priority over the default bot channel. Without an override, the
        default channel is used. Changes apply to future posts; queued deliveries keep their
        original destination.
      </p>
      {query.isPending && <p role="status">Loading categories…</p>}
      {query.isError && (
        <p role="alert" className="error">
          {query.error.message}
        </p>
      )}
      <div className="category-routes">
        {rows.map((row) => (
          <button
            type="button"
            key={row.category}
            className={`category-route ${category === row.category ? 'active' : ''}`}
            onClick={() => {
              setCategory(row.category);
              setChannel(row.channel_id || '');
              setSaved(false);
            }}
          >
            <span>
              <strong>{row.category}</strong>
              <small>
                {row.artists} {row.artists === 1 ? 'artist' : 'artists'}
              </small>
            </span>
            <ArrowRight size={16} />
            <span>{row.channel_name ? `#${row.channel_name}` : 'Default channel'}</span>
          </button>
        ))}
      </div>
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          setError('');
          setSaved(false);
          try {
            await api(`/servers/${serverId}/categories`, 'PUT', { category, channel_id: channel });
            await client.invalidateQueries({ queryKey: ['community', serverId, 'categories'] });
            setSaved(true);
          } catch (e) {
            setError((e as Error).message);
          } finally {
            setBusy(false);
          }
        }}
      >
        <fieldset disabled={busy} className="category-route-form">
          <CategoryField
            required
            value={category}
            categories={rows.map((r) => r.category)}
            onChange={(value) => {
              setCategory(value);
              setChannel(rows.find((r) => r.category === value)?.channel_id || '');
              setSaved(false);
            }}
          />
          <label>
            Category channel ID
            <input
              aria-label="Category channel ID"
              pattern="[0-9]{1,20}"
              value={channel}
              onChange={(e) => {
                setChannel(e.target.value);
                setSaved(false);
              }}
              placeholder="Leave blank to use the default channel"
            />
            <small>The bot verifies that the channel belongs to this server.</small>
          </label>
          <button className="button primary" disabled={!category.trim()}>
            {busy ? 'Saving…' : 'Save category channel'}
          </button>
        </fieldset>
        {saved && <p role="status">Category channel saved.</p>}
        {error && (
          <p role="alert" className="error">
            {error}
          </p>
        )}
      </form>
    </section>
  );
}
