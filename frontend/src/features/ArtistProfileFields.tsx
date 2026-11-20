import { useState } from 'react';
import { ImageOff, Plus, Trash2 } from 'lucide-react';

export interface ArtistLink {
  label: string;
  url: string;
}
export default function ArtistProfileFields({
  name,
  avatar,
  links,
  onAvatar,
  onLinks,
}: {
  name: string;
  avatar: string;
  links: ArtistLink[];
  onAvatar: (value: string) => void;
  onLinks: (value: ArtistLink[]) => void;
}) {
  const [failed, setFailed] = useState('');
  return (
    <section className="artist-profile-fields" aria-label="Artist photo and official pages">
      <div className="artist-profile-preview">
        {avatar && failed !== avatar ? (
          <img
            src={avatar}
            referrerPolicy="no-referrer"
            alt="Artist preview"
            onError={() => setFailed(avatar)}
          />
        ) : (
          <ImageOff size={30} />
        )}
        <div>
          <strong>{name || 'Artist profile'}</strong>
          <p>
            {avatar && failed === avatar
              ? 'The image could not be loaded. Check the URL.'
              : 'Use a direct HTTPS image URL.'}
          </p>
        </div>
      </div>
      <label>
        Photo URL
        <input
          aria-label="Photo URL"
          type="url"
          pattern="https://.*"
          maxLength={2048}
          placeholder="https://example.com/avatar.jpg"
          value={avatar}
          onChange={(e) => onAvatar(e.target.value)}
        />
        <small>Leave empty to use the artist’s initial.</small>
      </label>
      <div className="artist-links-heading">
        <h3>Official pages</h3>
        <button
          type="button"
          className="button secondary"
          disabled={links.length >= 8}
          onClick={() => onLinks([...links, { label: '', url: '' }])}
        >
          <Plus size={15} />
          Add link
        </button>
      </div>
      <p className="hint">Add up to eight official artist pages using HTTPS links.</p>
      {links.map((link, index) => (
        <div className="artist-link-editor" key={index}>
          <label>
            Label
            <input
              aria-label={`Link ${index + 1} label`}
              required
              maxLength={40}
              value={link.label}
              onChange={(e) =>
                onLinks(links.map((l, i) => (i === index ? { ...l, label: e.target.value } : l)))
              }
            />
          </label>
          <label>
            URL
            <input
              aria-label={`Link ${index + 1} URL`}
              required
              type="url"
              pattern="https://.*"
              maxLength={2048}
              value={link.url}
              onChange={(e) =>
                onLinks(links.map((l, i) => (i === index ? { ...l, url: e.target.value } : l)))
              }
            />
          </label>
          <button
            type="button"
            className="icon-button"
            aria-label={`Remove link ${index + 1}`}
            onClick={() => onLinks(links.filter((_, i) => i !== index))}
          >
            <Trash2 size={16} />
          </button>
        </div>
      ))}
    </section>
  );
}
