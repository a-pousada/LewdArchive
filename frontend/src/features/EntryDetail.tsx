import { useState } from 'react';
import {
  ArrowDownToLine,
  Bookmark,
  ChevronLeft,
  ChevronRight,
  ExternalLink,
  ImageOff,
  Send,
  LoaderCircle,
} from 'lucide-react';
import { date } from '../api';
import type { Entry } from '../api';
import { Modal, Source } from '../components';
export default function EntryDetail({
  entry: e,
  onClose,
  onDownload,
  onFavorite,
  onSendDiscord,
}: {
  entry: Entry;
  onClose: () => void;
  onDownload: () => void;
  onFavorite: () => void;
  onSendDiscord: () => Promise<string | null>;
}) {
  const [index, setIndex] = useState(0);
  const [broken, setBroken] = useState(false);
  const [sending, setSending] = useState(false);
  const [sendResult, setSendResult] = useState<string | null>(null);
  const media = e.media[index];
  return (
    <Modal
      wide
      title={e.title}
      subtitle={`${e.author} · ${date(e.published_at)}`}
      onClose={onClose}
    >
      {media && (
        <div className="detail-media">
          {broken ? (
            <div className="cover-fallback">
              <ImageOff size={40} />
              <p>The source did not allow this media to load.</p>
              <a href={media.url} target="_blank" rel="noreferrer" className="button secondary">
                Open media
                <ExternalLink size={14} />
              </a>
            </div>
          ) : media.kind === 'video' ? (
            <video controls src={media.url} onError={() => setBroken(true)} />
          ) : (
            <img
              src={media.url}
              alt={e.title}
              referrerPolicy="no-referrer"
              onError={() => setBroken(true)}
            />
          )}{' '}
          {e.media.length > 1 && (
            <div className="media-navigation">
              <button
                className="icon-button"
                disabled={index === 0}
                aria-label="Previous media"
                onClick={() => {
                  setIndex((i) => i - 1);
                  setBroken(false);
                }}
              >
                <ChevronLeft size={20} />
              </button>
              <span>
                {index + 1} / {e.media.length}
              </span>
              <button
                className="icon-button"
                disabled={index === e.media.length - 1}
                aria-label="Next media"
                onClick={() => {
                  setIndex((i) => i + 1);
                  setBroken(false);
                }}
              >
                <ChevronRight size={20} />
              </button>
            </div>
          )}
        </div>
      )}
      <div className="detail-copy">
        <Source id={e.source} />
        <p>{e.summary || 'This post has no description.'}</p>
      </div>
      <div className="modal-actions detail-actions">
        <button
          className="button secondary"
          disabled={sending}
          onClick={async () => {
            setSending(true);
            setSendResult(null);
            try {
              const error = await onSendDiscord();
              setSendResult(error ?? 'Discord test queued. Check Activity for the result.');
            } finally {
              setSending(false);
            }
          }}
        >
          {sending ? <LoaderCircle size={16} className="spin" /> : <Send size={16} />}
          Test on Discord
        </button>
        <button className="button secondary" onClick={onFavorite}>
          <Bookmark size={16} fill={e.favorite ? 'currentColor' : 'none'} />
          {e.favorite ? 'Favorited' : 'Favorite'}
        </button>
        <button className="button secondary" disabled={!e.media.length} onClick={onDownload}>
          <ArrowDownToLine size={16} />
          Download files
        </button>
        <a className="button primary" href={e.url} target="_blank" rel="noreferrer">
          Original post
          <ExternalLink size={16} />
        </a>
      </div>
      {sendResult && (
        <p className="hint" role="status">
          {sendResult}
        </p>
      )}
    </Modal>
  );
}
