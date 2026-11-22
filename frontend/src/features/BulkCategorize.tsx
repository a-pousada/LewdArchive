import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { CheckSquare, LoaderCircle, Sparkles, X } from 'lucide-react';
import { CategoryField, defaultCategories } from './Categories';
import { communityApi as api } from './communityApi';
import type { Artist } from './communityApi';

const legacyPlatformSuggestions: Record<string, string> = {
  Gelbooru: '2D Illustration',
  Fanbox: '2D Illustration',
  Onlyfans: 'Photography',
  Fansly: 'Photography',
  Patreon: '2D Illustration',
  X: '2D Illustration',
  Bluesky: '2D Illustration',
  Mastodon: '2D Illustration',
};

export default function BulkCategorize({
  serverId,
  artists,
  visibleArtists,
  categories,
  selected,
  onSelectedChange,
  onApplied,
  onClose,
}: {
  serverId: string;
  artists: Artist[];
  visibleArtists: Artist[];
  categories: string[];
  selected: Set<string>;
  onSelectedChange: (next: Set<string>) => void;
  onApplied: (message: string) => void;
  onClose: () => void;
}) {
  const client = useQueryClient();
  const [targetCategory, setTargetCategory] = useState('2D Illustration');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  const legacyGroups = Object.entries(
    artists.reduce<Record<string, Artist[]>>((acc, artist) => {
      if (artist.category && artist.category in legacyPlatformSuggestions) {
        (acc[artist.category] ||= []).push(artist);
      }
      return acc;
    }, {}),
  ).sort((a, b) => b[1].length - a[1].length);

  async function apply() {
    if (selected.size === 0) return;
    setBusy(true);
    setError('');
    try {
      const result = await api<{ updated: number; category: string }>(
        `/servers/${serverId}/artists/bulk-category`,
        'POST',
        {
          artist_ids: [...selected],
          category: targetCategory.trim(),
        },
      );
      onSelectedChange(new Set());
      await client.invalidateQueries({ queryKey: ['community', serverId] });
      const label = result.category || 'Uncategorized';
      onApplied(`Updated ${result.updated} artist(s) to "${label}".`);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="settings-card bulk-categorize-panel" aria-label="Bulk categorize artists">
      <div className="bulk-categorize-header">
        <div>
          <h2>Bulk categorize artists</h2>
          <p className="hint">
            {selected.size} selected of {visibleArtists.length} visible ({artists.length} total)
          </p>
        </div>
        <button
          type="button"
          className="icon-button"
          aria-label="Close bulk categorize"
          onClick={onClose}
        >
          <X size={16} />
        </button>
      </div>

      <div className="bulk-categorize-actions">
        <button
          type="button"
          className="button secondary"
          disabled={busy || visibleArtists.length === 0}
          onClick={() => onSelectedChange(new Set(visibleArtists.map((a) => a.id)))}
        >
          <CheckSquare size={15} />
          Select all visible ({visibleArtists.length})
        </button>
        <button
          type="button"
          className="button secondary"
          disabled={busy || selected.size === 0}
          onClick={() => onSelectedChange(new Set())}
        >
          Clear selection
        </button>
      </div>

      {legacyGroups.length > 0 && (
        <div className="bulk-legacy-groups">
          <small>
            <Sparkles size={13} /> Legacy platform labels detected — pick one to review and assign:
          </small>
          <div className="bulk-categorize-presets">
            {legacyGroups.map(([label, group]) => {
              const suggested = legacyPlatformSuggestions[label];
              return (
                <button
                  key={label}
                  type="button"
                  className="category-chip"
                  disabled={busy}
                  onClick={() => {
                    onSelectedChange(new Set(group.map((a) => a.id)));
                    if (suggested) setTargetCategory(suggested);
                  }}
                >
                  Select {label} ({group.length}) → {suggested}
                </button>
              );
            })}
          </div>
        </div>
      )}

      <div className="bulk-categorize-presets">
        {defaultCategories.map((preset) => (
          <button
            key={preset}
            type="button"
            className={`category-chip ${targetCategory === preset ? 'active' : ''}`}
            disabled={busy}
            onClick={() => setTargetCategory(preset)}
          >
            {preset}
          </button>
        ))}
        <button
          type="button"
          className={`category-chip ${targetCategory === '' ? 'active' : ''}`}
          disabled={busy}
          onClick={() => setTargetCategory('')}
        >
          Uncategorized
        </button>
      </div>

      <div className="bulk-categorize-controls">
        <CategoryField
          value={targetCategory}
          onChange={setTargetCategory}
          categories={categories}
        />
        <button
          type="button"
          className="button primary"
          disabled={busy || selected.size === 0}
          onClick={() => void apply()}
        >
          {busy ? <LoaderCircle className="spin" size={16} /> : null}
          Apply to {selected.size} {selected.size === 1 ? 'artist' : 'artists'}
        </button>
      </div>

      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
