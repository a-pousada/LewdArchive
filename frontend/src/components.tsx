import { useEffect, useRef, useId } from 'react';
import type { ReactNode } from 'react';
import { X, ArrowUpRight, Plus, Radio } from 'lucide-react';
import { sources } from './api';
export function Logo() {
  return (
    <div className="logo">
      <img src="/favicon.svg" alt="" />
      <span>
        Lewd<span className="logo-light">Archive</span>
        <small>THE ARTIST OBSERVATORY</small>
      </span>
    </div>
  );
}
export function Source({ id }: { id: string }) {
  const source = sources.find((s) => s.id === id) || sources[0];
  return (
    <span className={`source source-${source.id}`}>
      <b>{source.mark}</b>
      {source.label}
    </span>
  );
}
export function Toggle({
  checked,
  onChange,
  label,
  description,
  disabled = false,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  description?: string;
  disabled?: boolean;
}) {
  return (
    <label className="toggle-row">
      <span>
        <strong>{label}</strong>
        {description && <small>{description}</small>}
      </span>
      <input
        className="switch"
        aria-label={label}
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        disabled={disabled}
      />
    </label>
  );
}
export function Modal({
  title,
  subtitle,
  children,
  onClose,
  wide = false,
}: {
  title: string;
  subtitle?: string;
  children: ReactNode;
  onClose: () => void;
  wide?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  useEffect(() => {
    const d = ref.current!;
    d.showModal();
    return () => d.close();
  }, []);
  return (
    <dialog
      ref={ref}
      aria-labelledby={titleId}
      className={`modal ${wide ? 'modal-wide' : ''}`}
      onCancel={onClose}
      onClick={(e) => {
        if (e.target === ref.current) onClose();
      }}
    >
      <div className="modal-head">
        <div>
          <span className="eyebrow">LEWDARCHIVE / COLLECTION</span>
          <h2 id={titleId}>{title}</h2>
          {subtitle && <p>{subtitle}</p>}
        </div>
        <button className="icon-button" onClick={onClose} aria-label="Close">
          <X size={20} />
        </button>
      </div>
      {children}
    </dialog>
  );
}
export function EmptyArt() {
  return (
    <svg className="empty-art" viewBox="0 0 380 240" fill="none" aria-hidden="true">
      <circle cx="197" cy="117" r="92" stroke="#393039" strokeDasharray="3 7" />
      <circle cx="197" cy="117" r="68" fill="#251f28" />
      <path d="M259 31c-21 7-24 33-9 47-33 0-44-43-17-58 8-4 19-1 26 11Z" fill="#d595a9" />
      <g transform="rotate(-12 115 143)">
        <rect x="62" y="61" width="117" height="150" rx="8" fill="#1b1b22" stroke="#51414d" />
        <path d="M73 73h95v104H73Z" fill="#302936" />
        <path d="m77 163 24-55 19 25 21-36 23 66" stroke="#786078" strokeWidth="2" />
        <circle cx="105" cy="97" r="10" fill="#c0889f" />
        <path d="M78 189h54M78 199h34" stroke="#715a6a" strokeWidth="3" />
      </g>
      <g transform="rotate(8 216 146)">
        <rect x="145" y="65" width="131" height="160" rx="8" fill="#212027" stroke="#927085" />
        <path d="M156 77h109v111H156Z" fill="#36303f" />
        <path
          d="M166 183c1-23 8-45 17-62l-1-25 18 11c8-3 21-3 30 0l18-11-2 29c9 16 14 39 14 58"
          fill="#b4879f"
        />
        <path d="m183 128 17 4m31-4-16 4" stroke="#352735" strokeWidth="3" strokeLinecap="round" />
        <path d="m204 143 5 4 5-4" stroke="#352735" strokeWidth="2" />
        <path d="M160 202h58M160 212h37" stroke="#a37d90" strokeWidth="3" />
      </g>
      <path d="m298 99 4 10 11 4-11 4-4 11-4-11-10-4 10-4Z" fill="#c899a9" />
      <path d="m89 29 2 6 7 2-7 3-2 6-3-6-6-3 6-2Z" fill="#c899a9" />
      <circle cx="310" cy="172" r="3" fill="#8b7385" />
    </svg>
  );
}
export function Empty({ kind = 'library', onAdd }: { kind?: string; onAdd?: () => void }) {
  const texts: Record<string, [string, string]> = {
    library: [
      'Every collection starts with an artist.',
      'Add a feed and let LewdArchive keep track of new posts for you.',
    ],
    search: ['No posts here yet.', 'Try another search or adjust your library filters.'],
    favorites: ['Keep what inspires you.', 'Bookmark a post to add it to this collection.'],
    downloads: [
      'Your collection, offline too.',
      'Download files from a post or enable automatic downloads in the feed settings.',
    ],
    activity: ['All quiet for now.', 'Notifications and downloads for new posts will appear here.'],
    feeds: ['Your artists, all in one place.', 'Connect an RSS or Atom feed to start following.'],
  };
  const [title, description] = texts[kind] || texts.library;
  return (
    <div className="empty">
      <EmptyArt />
      <span className="eyebrow">A SPACE FOR YOUR DISCOVERIES</span>
      <h2>{title}</h2>
      <p>{description}</p>
      {onAdd && (
        <button className="button primary" onClick={onAdd}>
          <Plus size={17} />
          Add your first feed
          <ArrowUpRight size={16} />
        </button>
      )}
      <div className="empty-foot">
        <Radio size={13} /> RSS · ATOM · RSSHUB
      </div>
    </div>
  );
}
