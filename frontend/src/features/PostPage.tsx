import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Bookmark, Heart, Link, ExternalLink, ImageOff } from 'lucide-react';
import { communityApi as api } from './communityApi';
import type { Post } from './communityApi';
import { date } from '../api';
import { CommentThread } from './PostComments';
import PostFiles from './PostFiles';
import PostTags from './PostTags';
export function postUrl(server: string, post: string) {
  return `/community?server=${encodeURIComponent(server)}&post=${encodeURIComponent(post)}`;
}
export default function PostPage({ serverId, postId }: { serverId: string; postId: string }) {
  const client = useQueryClient();
  const [error, setError] = useState('');
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);
  const [manual, setManual] = useState(false);
  const key = ['community', serverId, 'post', postId];
  const data = useQuery({
    queryKey: key,
    queryFn: () => api<{ post: Post; can_manage: boolean }>(`/servers/${serverId}/posts/${postId}`),
    retry: false,
  });
  const back = (
    <a className="button secondary" href={`/community?server=${encodeURIComponent(serverId)}`}>
      <ArrowLeft size={16} />
      Back to library
    </a>
  );
  if (data.isPending)
    return (
      <>
        {back}
        <p role="status">Loading artwork…</p>
      </>
    );
  if (data.isError)
    return (
      <>
        {back}
        <section className="community-empty">
          <ImageOff size={32} />
          <h1>Post unavailable</h1>
          <p role="alert">{data.error.message}</p>
          <p>You must be a current member of this server to view this post.</p>
        </section>
      </>
    );
  const p = data.data.post;
  const url = new URL(postUrl(serverId, postId), location.origin).href;
  async function change(value: unknown) {
    setBusy(true);
    setError('');
    try {
      await api(`/servers/${serverId}/posts/${postId}`, 'PATCH', value);
      await client.invalidateQueries({ queryKey: key });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <article className="post-page">
      {back}
      <header className="post-page-heading">
        <span className="eyebrow">{p.feed_name}</span>
        <h1>{p.title}</h1>
        <p>
          {p.author} · {date(p.published_at)}
        </p>
        <div className="post-page-actions">
          <button
            className="button secondary"
            disabled={busy}
            aria-pressed={p.liked}
            onClick={() => void change({ liked: !p.liked })}
          >
            <Heart size={17} fill={p.liked ? 'currentColor' : 'none'} />
            {p.likes} likes
          </button>
          <button
            className="button secondary"
            disabled={busy}
            aria-pressed={p.favorite}
            onClick={() => void change({ favorite: !p.favorite })}
          >
            <Bookmark size={17} />
            {p.favorite ? 'Saved' : 'Save post'}
          </button>
          <button
            className="button primary"
            onClick={async () => {
              try {
                await navigator.clipboard.writeText(url);
                setCopied(true);
                setManual(false);
              } catch {
                setManual(true);
              }
            }}
          >
            <Link size={17} />
            {copied ? 'Link copied' : 'Share post'}
          </button>
          <a className="button secondary" href={p.url} target="_blank" rel="noreferrer">
            <ExternalLink size={16} />
            Original post
          </a>
        </div>
        <p className="hint">Shared links are visible only to members of this server.</p>
        {copied && <span role="status">Post link copied.</span>}
        {manual && (
          <label>
            Copy this link
            <input readOnly value={url} onFocus={(e) => e.target.select()} />
          </label>
        )}
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
      </header>
      {p.summary && <p className="post-page-summary">{p.summary}</p>}
      <PostTags key={p.id} serverId={serverId} post={p} manager={data.data.can_manage} />
      <section className="post-gallery" aria-label="Post media">
        <div className="post-gallery-heading">
          <h2>Artwork</h2>
          <span>{p.media.length} files</span>
        </div>
        {p.media.length === 0 && <p className="notice">This post has no attached media.</p>}
        {p.media.map((m, i) => (
          <figure key={m.url + i}>
            {m.kind === 'image' ? (
              <GalleryImage url={m.url} title={`${p.title} — image ${i + 1}`} />
            ) : m.kind === 'video' ? (
              <video
                controls
                preload="metadata"
                src={m.url}
                aria-label={`${p.title} — video ${i + 1}`}
              />
            ) : (
              <a href={m.url} target="_blank" rel="noreferrer">
                Open attachment {i + 1}
              </a>
            )}
            <figcaption>
              {i + 1} / {p.media.length}
              <a href={m.url} target="_blank" rel="noreferrer">
                Open original file ↗
              </a>
            </figcaption>
          </figure>
        ))}
      </section>
      <PostFiles serverId={serverId} post={p} manager={data.data.can_manage} />
      <section className="post-page-comments" aria-label="Post comments">
        <h2>Conversation</h2>
        <CommentThread serverId={serverId} post={p} />
      </section>
    </article>
  );
}
function GalleryImage({ url, title }: { url: string; title: string }) {
  const [failed, setFailed] = useState(false);
  return failed ? (
    <div className="gallery-fallback">
      <ImageOff size={30} />
      <p>Image unavailable. Try the original file below.</p>
    </div>
  ) : (
    <img
      src={url}
      alt={title}
      loading="lazy"
      referrerPolicy="no-referrer"
      onError={() => setFailed(true)}
    />
  );
}
