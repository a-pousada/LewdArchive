import { useState } from 'react';
import { ArrowUpRight, Bookmark, Check, Grid2X2, ImageOff } from 'lucide-react';
import { date } from '../api';
import type { Entry } from '../api';
import { Source } from '../components';
export default function EntryCard({
  entry: e,
  onOpen,
  onFavorite,
}: {
  entry: Entry;
  onOpen: () => void;
  onFavorite: () => void;
}) {
  const [broken, setBroken] = useState(false);
  const image = e.media.find((m) => m.kind === 'image');
  return (
    <article className={`entry-card ${e.is_read ? 'read' : ''}`}>
      <button className="entry-cover" onClick={onOpen} aria-label={`Open ${e.title}`}>
        {image && !broken ? (
          <img
            src={image.url}
            alt={e.title}
            loading="lazy"
            referrerPolicy="no-referrer"
            onError={() => setBroken(true)}
          />
        ) : (
          <div className="cover-fallback">
            <ImageOff size={32} />
            <span>
              {e.media.some((m) => m.kind === 'video') ? 'Video post' : 'Post without an image'}
            </span>
          </div>
        )}
        {!e.is_read && <span className="new-label">NEW</span>}
        {e.media.length > 1 && (
          <span className="media-count">
            <Grid2X2 size={12} />
            {e.media.length}
          </span>
        )}
      </button>
      <div className="entry-body">
        <div className="entry-byline">
          <Source id={e.source} />
          <button
            className={`icon-button ${e.favorite ? 'pink-text' : ''}`}
            aria-label={e.favorite ? 'Remove from favorites' : 'Save to favorites'}
            onClick={onFavorite}
          >
            <Bookmark size={17} fill={e.favorite ? 'currentColor' : 'none'} />
          </button>
        </div>
        <button className="entry-title" onClick={onOpen}>
          {e.title}
        </button>
        <div className="entry-author">
          <span>{e.author}</span>
          <ArrowUpRight size={13} />
        </div>
        <div className="entry-date">
          <span>{date(e.published_at)}</span>
          {e.is_read && <Check size={13} />}
        </div>
      </div>
    </article>
  );
}
