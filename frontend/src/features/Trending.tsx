import { postUrl } from './PostPage';
import PostComments from './PostComments';
import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  TrendingUp,
  Heart,
  Bookmark,
  ArrowUpRight,
  ArrowLeft,
  ArrowRight,
  ImageOff,
  RefreshCw,
  Trophy,
} from 'lucide-react';
import { communityApi as api } from './communityApi';
import type { Post, Page } from './communityApi';
type RankedPost = Post & { rank: number; saves: number; score: number; artist_avatar: string };
type Rankings = Omit<Page, 'items'> & {
  items: RankedPost[];
  days: number;
  summary: { posts: number; likes: number; saves: number; artists: number };
};
const periods = [
  { id: 'week', label: 'Week', days: 7 },
  { id: 'month', label: 'Month', days: 30 },
  { id: 'year', label: 'Year', days: 365 },
];
const metrics = [
  { id: 'likes', label: 'Most liked' },
  { id: 'saves', label: 'Most saved' },
  { id: 'engagement', label: 'Likes + saves' },
];
export default function Trending({ serverId }: { serverId: string }) {
  const client = useQueryClient();
  const [period, setPeriod] = useState('week');
  const [metric, setMetric] = useState('likes');
  const [page, setPage] = useState(1);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const data = useQuery({
    queryKey: ['community', serverId, 'trending', period, metric, page],
    queryFn: () =>
      api<Rankings>(`/servers/${serverId}/trending?period=${period}&metric=${metric}&page=${page}`),
  });
  async function preference(p: RankedPost, value: { liked?: boolean; favorite?: boolean }) {
    setBusy(true);
    setError('');
    try {
      await api(`/servers/${serverId}/posts/${p.id}`, 'PATCH', value);
      await client.invalidateQueries({ queryKey: ['community', serverId] });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="trending" aria-label="Trending artwork">
      <div className="trending-intro">
        <div>
          <span className="eyebrow">
            <TrendingUp size={14} />
            THE COMMUNITY EDIT
          </span>
          <h2>Worth a second look.</h2>
          <p>The artwork your server keeps coming back to.</p>
        </div>
        <div className="trending-seal">
          <Trophy size={26} />
          <span>
            Selected by
            <br />
            your community
          </span>
        </div>
      </div>
      <div className="trending-toolbar">
        <div className="trending-periods" aria-label="Ranking period">
          {periods.map((p) => (
            <button
              key={p.id}
              aria-pressed={period === p.id}
              onClick={() => {
                setPeriod(p.id);
                setPage(1);
              }}
            >
              {p.label}
              <small>{p.days} days</small>
            </button>
          ))}
        </div>
        <label>
          Rank by
          <select
            aria-label="Rank by"
            value={metric}
            onChange={(e) => {
              setMetric(e.target.value);
              setPage(1);
            }}
          >
            {metrics.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label}
              </option>
            ))}
          </select>
        </label>
      </div>
      <p className="trending-explainer">
        Posts published in the last {periods.find((p) => p.id === period)!.days} days · Current
        counts · Your server only
      </p>
      {data.data && (
        <div className="trending-stats">
          <div>
            <strong>{data.data.summary.posts.toLocaleString()}</strong>
            <span>Posts in this window</span>
          </div>
          <div>
            <strong>{data.data.summary.likes.toLocaleString()}</strong>
            <span>
              <Heart size={13} /> Likes
            </span>
          </div>
          <div>
            <strong>{data.data.summary.saves.toLocaleString()}</strong>
            <span>
              <Bookmark size={13} /> Saves
            </span>
          </div>
          <div>
            <strong>{data.data.summary.artists.toLocaleString()}</strong>
            <span>Artists represented</span>
          </div>
        </div>
      )}
      <p className="hint">
        Only totals are public. Who saved a post and their personal collections stay private.
      </p>
      {data.isPending && (
        <div className="trending-loading" role="status">
          <RefreshCw className="spin" size={20} />
          Finding this period’s favorites…
        </div>
      )}
      {data.isError && (
        <div className="notice" role="alert">
          Could not load rankings.{' '}
          <button className="button secondary" onClick={() => void data.refetch()}>
            Try again
          </button>
        </div>
      )}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {data.data?.items.length === 0 && (
        <div className="community-empty">
          <TrendingUp size={40} />
          <h3>A new chart is waiting to happen.</h3>
          <p>
            No posts have{' '}
            {metric === 'likes' ? 'likes' : metric === 'saves' ? 'saves' : 'likes or saves'} in this
            publication window. Explore recent artwork or choose a longer period.
          </p>
        </div>
      )}
      <div className="trending-grid">
        {data.data?.items.map((p) => (
          <article
            key={p.id}
            className={p.rank === 1 ? 'trending-card trending-leader' : 'trending-card'}
          >
            <a className="trending-art" href={postUrl(serverId, p.id)}>
              <span className="trending-rank">
                <small>NO.</small>
                {String(p.rank).padStart(2, '0')}
              </span>
              {p.media.find((m) => m.kind === 'image') ? (
                <img
                  alt={p.title}
                  src={p.media.find((m) => m.kind === 'image')!.url}
                  loading="lazy"
                  referrerPolicy="no-referrer"
                />
              ) : (
                <ImageOff size={40} />
              )}
              <span className="trending-open">
                <ArrowUpRight size={20} />
              </span>
            </a>
            <div className="trending-card-body">
              <div className="trending-byline">
                {p.artist_avatar && (
                  <img src={p.artist_avatar} alt="" referrerPolicy="no-referrer" />
                )}
                <span>{p.feed_name}</span>
                {p.rank === 1 && <span className="trending-pick">TOP PICK</span>}
              </div>
              <h3>
                <a href={postUrl(serverId, p.id)}>{p.title}</a>
              </h3>
              <PostComments serverId={serverId} post={p} />
              <div className="trending-actions">
                <button
                  disabled={busy}
                  aria-label={p.liked ? 'Unlike post' : 'Like post'}
                  aria-pressed={p.liked}
                  onClick={() => void preference(p, { liked: !p.liked })}
                >
                  <Heart size={17} fill={p.liked ? 'currentColor' : 'none'} />
                  {p.likes}
                  <span>likes</span>
                </button>
                <button
                  disabled={busy}
                  aria-label={p.favorite ? 'Unsave post' : 'Save post'}
                  aria-pressed={p.favorite}
                  onClick={() => void preference(p, { favorite: !p.favorite })}
                >
                  <Bookmark size={17} fill={p.favorite ? 'currentColor' : 'none'} />
                  {p.saves}
                  <span>saves</span>
                </button>
              </div>
            </div>
          </article>
        ))}
      </div>
      {data.data && data.data.total > 0 && (
        <div className="community-pagination">
          <span>
            {data.data.total} ranked posts · Page {page} of {data.data.pages}
          </span>
          <button
            className="button secondary"
            disabled={page <= 1 || data.isFetching}
            onClick={() => setPage((p) => p - 1)}
          >
            <ArrowLeft size={16} />
            Previous
          </button>
          <button
            className="button secondary"
            disabled={page >= data.data.pages || data.isFetching}
            onClick={() => setPage((p) => p + 1)}
          >
            Next
            <ArrowRight size={16} />
          </button>
        </div>
      )}
    </section>
  );
}
