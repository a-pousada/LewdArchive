import PostPage, { postUrl } from './PostPage';
import FeedTransfer from './FeedTransfer';
import PostFiles from './PostFiles';
import PostComments from './PostComments';
import WebhookManager from './WebhookManager';
import WebhookActivity from './WebhookActivity';
import FeedHealth from './FeedHealth';
import AiTagActivity from './AiTagActivity';
import Trending from './Trending';
import ArtistProfile from './ArtistProfile';
import BulkCategorize from './BulkCategorize';
import Categories, { useCategories } from './Categories';
import { TagBadges } from './PostTags';
import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Archive,
  TrendingUp,
  Pencil,
  Heart,
  Bookmark,
  Bell,
  Users,
  Settings2,
  Activity,
  Plus,
  ExternalLink,
  LogOut,
  ArrowLeft,
  ArrowRight,
  ImageOff,
  LoaderCircle,
  CheckSquare,
} from 'lucide-react';
import { Logo, Toggle } from '../components';
import { ApiError, date } from '../api';
import FeedForm from './FeedForm';
import { communityApi as api } from './communityApi';
import type { Member, ServerChoice, Server, Artist, Post, Page, Delivery } from './communityApi';

type View =
  | 'trending'
  | 'posts'
  | 'artists'
  | 'saved'
  | 'favorite-artists'
  | 'liked'
  | 'settings'
  | 'activity';
const views: { id: View; label: string; icon: typeof Heart; manager?: boolean }[] = [
  { id: 'trending', label: 'Trending', icon: TrendingUp },
  { id: 'posts', label: 'Latest artwork', icon: Archive },
  { id: 'artists', label: 'Artists', icon: Users },
  { id: 'saved', label: 'Saved posts', icon: Bookmark },
  { id: 'favorite-artists', label: 'Favorite artists', icon: Bookmark },
  { id: 'liked', label: 'Liked posts', icon: Heart },
  { id: 'settings', label: 'Server settings', icon: Settings2, manager: true },
  { id: 'activity', label: 'Delivery activity', icon: Activity, manager: true },
];
export default function CommunityApp() {
  const client = useQueryClient();
  const config = useQuery({
    queryKey: ['community', 'config'],
    queryFn: () => api<{ discord_enabled: boolean; install_url: string | null }>('/config'),
  });
  const me = useQuery({
    queryKey: ['community', 'me'],
    queryFn: () => api<Member>('/me'),
    retry: false,
  });
  const servers = useQuery({
    queryKey: ['community', 'servers'],
    queryFn: () => api<ServerChoice[]>('/servers'),
    enabled: !!me.data,
  });
  const route = new URLSearchParams(location.search);
  const routeServer = route.get('server') || '';
  const routePost = route.get('post') || '';
  const [selected, setSelected] = useState(routeServer);
  useEffect(() => {
    if (!me.data) return;
    try {
      const pending = sessionStorage.getItem('community-return');
      if (routePost) {
        sessionStorage.removeItem('community-return');
        return;
      }
      if (
        pending?.startsWith('/community?') &&
        !new URLSearchParams(location.search).has('login_error')
      ) {
        sessionStorage.removeItem('community-return');
        location.replace(pending);
      }
    } catch {
      /* Navigation still works when browser storage is unavailable. */
    }
  }, [me.data, routePost]);
  const [error, setError] = useState('');
  const connect = useMutation({
    mutationFn: (external_id: string) => api<Server>('/servers', 'POST', { external_id }),
    onSuccess: async (s) => {
      await client.invalidateQueries({ queryKey: ['community', 'servers'] });
      setSelected(s.id);
    },
    onError: (e) => setError(e.message),
  });
  const current = selected || servers.data?.find((s) => s.connected)?.id || '';
  if (me.isPending || config.isPending)
    return (
      <div className="boot">
        <Logo />
        <LoaderCircle className="spin" />
      </div>
    );
  if (!me.data)
    return (
      <main className="community-welcome">
        <Logo />
        <span className="eyebrow">YOUR SERVER’S ART COLLECTION</span>
        <h1>
          Artists you follow.
          <br />
          Artwork you keep.
        </h1>
        <p>
          Discover new posts with your community. Keep a private collection of favorites and choose
          which artists can notify you.
        </p>
        {config.data?.discord_enabled ? (
          <a
            className="button primary"
            href="/api/community/auth/login"
            onClick={() => {
              try {
                if (routeServer && routePost)
                  sessionStorage.setItem('community-return', postUrl(routeServer, routePost));
              } catch {
                /* Storage is optional. */
              }
            }}
          >
            Continue with Discord <ArrowRight size={18} />
          </a>
        ) : (
          <div className="notice">
            The Discord integration is not configured. Ask the installation administrator to
            complete the bot setup.
          </div>
        )}
        {(new URLSearchParams(location.search).has('login_error') ||
          (me.error instanceof ApiError && me.error.status !== 401) ||
          config.isError) && (
          <p className="error" role="alert">
            Could not complete sign-in. Please try again.
          </p>
        )}
        <a className="community-admin" href="/">
          Installation administrator →
        </a>
      </main>
    );
  return (
    <div className="community-shell">
      <header className="community-header">
        <a href="/community" aria-label="Community home">
          <Logo />
        </a>
        <span className="community-tag">COMMUNITY</span>
        <div className="community-account">
          {me.data.avatar_url && (
            <img src={me.data.avatar_url} alt="" referrerPolicy="no-referrer" />
          )}
          <span>{me.data.display_name}</span>
          <button
            className="icon-button"
            aria-label="Sign out"
            onClick={async () => {
              try {
                await api('/auth/logout', 'POST');
                client.clear();
                location.assign('/community');
              } catch (e) {
                setError((e as Error).message);
              }
            }}
          >
            <LogOut size={18} />
          </button>
        </div>
      </header>
      <div className="community-layout">
        <aside className="community-sidebar">
          <label>
            Your server
            <select
              aria-label="Your server"
              value={current}
              onChange={(e) => {
                if (routePost)
                  location.assign(`/community?server=${encodeURIComponent(e.target.value)}`);
                else setSelected(e.target.value);
              }}
            >
              <option value="" disabled>
                Select a server
              </option>
              {servers.data
                ?.filter((s) => s.connected)
                .map((s) => (
                  <option key={s.external_id} value={s.id!}>
                    {s.name}
                  </option>
                ))}
            </select>
          </label>
          <p className="hint">Favorites are private. Like counts are shared within each server.</p>
          <details className="community-connect">
            <summary>Connect another server</summary>
            <p>You need Administrator permission. Invite the bot before connecting.</p>
            {config.data?.install_url && (
              <a
                href={config.data.install_url}
                target="_blank"
                rel="noreferrer"
                className="button secondary"
              >
                Invite bot <ExternalLink size={14} />
              </a>
            )}
            {servers.data
              ?.filter((s) => !s.connected && s.can_manage)
              .map((s) => (
                <button
                  className="button secondary"
                  key={s.external_id}
                  disabled={connect.isPending}
                  onClick={() => {
                    setError('');
                    connect.mutate(s.external_id);
                  }}
                >
                  {s.name} <Plus size={14} />
                </button>
              ))}
            <p className="hint">Joined a new server? Sign out and sign in to refresh this list.</p>
          </details>
          <a className="community-admin" href="/">
            Installation administration ↗
          </a>
        </aside>
        <main className="community-main">
          {error && (
            <p role="alert" className="error">
              {error}
            </p>
          )}
          {servers.isError && (
            <p role="alert" className="error">
              {servers.error.message}
            </p>
          )}
          {current ? (
            routePost ? (
              <PostPage serverId={current} postId={routePost} />
            ) : (
              <Library key={current} serverId={current} />
            )
          ) : (
            <div className="community-empty">
              <Users size={36} />
              <h1>Your community starts here.</h1>
              <p>
                Connect a server from the sidebar, or ask a server administrator to invite and
                configure the bot.
              </p>
            </div>
          )}
        </main>
      </div>
    </div>
  );
}
function Library({ serverId }: { serverId: string }) {
  const client = useQueryClient();
  const [view, setView] = useState<View>('posts');
  const [category, setCategory] = useState('*');
  const [tag, setTag] = useState('');
  const [tagDraft, setTagDraft] = useState('');
  const [artistSearch, setArtistSearch] = useState('');
  const [bulkMode, setBulkMode] = useState(false);
  const [selectedArtists, setSelectedArtists] = useState<Set<string>>(() => new Set());
  const [bulkStatus, setBulkStatus] = useState('');
  const categories = useCategories(serverId);
  const categoryRows = Array.isArray(categories.data) ? categories.data : [];
  const [page, setPage] = useState(1);
  const [adding, setAdding] = useState(false);
  const [editingArtist, setEditingArtist] = useState<Artist | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setBulkMode(false);
    setSelectedArtists(new Set());
    setBulkStatus('');
    setArtistSearch('');
  }, [serverId]);
  const base = `/servers/${serverId}`;
  const server = useQuery({
    queryKey: ['community', serverId, 'server'],
    queryFn: () => api<{ server: Server; can_manage: boolean }>(base),
  });
  const artistView = view === 'artists' || view === 'favorite-artists';
  const postView = ['posts', 'saved', 'liked'].includes(view);
  const posts = useQuery({
    queryKey: ['community', serverId, view, page, category, tag],
    queryFn: () =>
      api<Page>(
        `${base}/posts?page=${page}&favorites=${view === 'saved'}&liked=${view === 'liked'}${category === '*' ? '' : `&category=${encodeURIComponent(category)}`}&tag=${encodeURIComponent(tag)}`,
      ),
    enabled: !!server.data && postView,
  });
  const artists = useQuery({
    queryKey: ['community', serverId, 'artists', view],
    queryFn: () => api<Artist[]>(`${base}/artists?favorites=${view === 'favorite-artists'}`),
    enabled: !!server.data && artistView,
  });
  const activity = useQuery({
    queryKey: ['community', serverId, 'activity'],
    queryFn: () => api<Delivery[]>(`${base}/deliveries`),
    enabled: !!server.data?.can_manage && view === 'activity',
    refetchInterval: view === 'activity' ? 15000 : false,
  });
  const act = async (path: string, method: string, body?: unknown) => {
    setBusy(true);
    setError('');
    try {
      await api(base + path, method, body);
      await client.invalidateQueries({ queryKey: ['community', serverId] });
      return true;
    } catch (e) {
      setError((e as Error).message);
      return false;
    } finally {
      setBusy(false);
    }
  };
  if (server.isPending) return <p role="status">Loading your server…</p>;
  if (server.isError)
    return (
      <div className="notice" role="alert">
        {server.error.message}
        <button className="button secondary" onClick={() => void server.refetch()}>
          Try again
        </button>
      </div>
    );
  const manager = server.data?.can_manage;
  const title = views.find((v) => v.id === view)!.label;
  const normalizedArtistSearch = artistSearch.trim().toLowerCase();
  const filteredArtists =
    artists.data?.filter(
      (a) =>
        (category === '*' || (a.category || '') === category) &&
        (!normalizedArtistSearch ||
          a.name.toLowerCase().includes(normalizedArtistSearch) ||
          a.source.toLowerCase().includes(normalizedArtistSearch) ||
          (a.category || 'uncategorized').toLowerCase().includes(normalizedArtistSearch)),
    ) ?? [];
  return (
    <>
      <div className="community-heading">
        <div>
          <span className="eyebrow">{server.data?.server.name}</span>
          <h1>{title}</h1>
          <p>
            {view === 'saved' || view === 'favorite-artists'
              ? 'Only you can see this collection.'
              : 'A shared library, curated by your server.'}
          </p>
        </div>
        {manager && (
          <button className="button primary" onClick={() => setAdding(true)}>
            <Plus size={16} />
            Track artist
          </button>
        )}
      </div>
      <nav className="community-tabs" aria-label="Server library">
        {views
          .filter((v) => !v.manager || manager)
          .map((v) => (
            <button
              key={v.id}
              className={view === v.id ? 'active' : ''}
              onClick={() => {
                setView(v.id);
                setPage(1);
                setError('');
                setBulkMode(false);
                setSelectedArtists(new Set());
                setBulkStatus('');
              }}
            >
              <v.icon size={16} />
              {v.label}
            </button>
          ))}
      </nav>
      {error && (
        <p role="alert" className="error">
          {error}
        </p>
      )}
      {((postView && posts.isError) ||
        (artistView && artists.isError) ||
        (view === 'activity' && activity.isError)) && (
        <p className="error" role="alert">
          {posts.error?.message || artists.error?.message || activity.error?.message}
        </p>
      )}
      {((postView && posts.isFetching) || (artistView && artists.isFetching)) && (
        <p className="hint" role="status">
          Updating collection…
        </p>
      )}
      {(postView || artistView) && (
        <form
          className="library-filters"
          onSubmit={(e) => {
            e.preventDefault();
            setTag(tagDraft.trim().replace(/^#/, '').toLowerCase().replaceAll(/\s+/g, '_'));
            setPage(1);
          }}
        >
          <label>
            Category
            <select
              aria-label="Category"
              value={category}
              onChange={(e) => {
                setCategory(e.target.value);
                setPage(1);
              }}
            >
              <option value="*">All categories</option>
              <option value="">Uncategorized</option>
              {categoryRows.map((r) => (
                <option key={r.category} value={r.category}>
                  {r.category}
                </option>
              ))}
            </select>
          </label>
          {postView && (
            <>
              <label>
                Filter by tag
                <input
                  value={tagDraft}
                  onChange={(e) => setTagDraft(e.target.value)}
                  placeholder="e.g. digital_art"
                  maxLength={48}
                />
              </label>
              <button className="button secondary">Apply filter</button>
            </>
          )}
          {artistView && (
            <label>
              Find artist
              <input
                aria-label="Find artist"
                value={artistSearch}
                onChange={(e) => setArtistSearch(e.target.value)}
                placeholder="Name, source, or category"
                maxLength={80}
              />
            </label>
          )}
          {(category !== '*' || tag || (artistView && artistSearch)) && (
            <button
              type="button"
              className="button secondary"
              onClick={() => {
                setCategory('*');
                setTag('');
                setTagDraft('');
                setArtistSearch('');
                setPage(1);
              }}
            >
              Clear filters
            </button>
          )}
          {artistView && manager && (
            <button
              type="button"
              className={bulkMode ? 'button selected' : 'button secondary'}
              onClick={() => {
                setBulkMode((v) => !v);
                setBulkStatus('');
              }}
            >
              <CheckSquare size={15} />
              {bulkMode ? 'Done categorizing' : 'Bulk categorize'}
            </button>
          )}
        </form>
      )}
      {postView && posts.data && (
        <>
          <div className="community-grid">
            {posts.data.items.map((p) => (
              <PostCard
                key={p.id}
                post={p}
                serverId={serverId}
                manager={!!manager}
                busy={busy}
                change={(value) => void act(`/posts/${p.id}`, 'PATCH', value)}
              />
            ))}
          </div>
          {!posts.data.items.length && (
            <div className="community-empty">
              <Bookmark size={30} />
              <h2>{category !== '*' || tag ? 'No matching posts.' : 'Nothing here yet.'}</h2>
              <p>
                {category !== '*' || tag
                  ? 'Try another category or tag, or clear the filters.'
                  : view === 'posts'
                    ? 'Newly tracked artists will appear after their first feed sync.'
                    : 'Save or like artwork from the latest posts to build your collection.'}
              </p>
            </div>
          )}
          <div className="community-pagination">
            <span>
              {posts.data.total} posts · Page {page} of {Math.max(1, posts.data.pages)}
            </span>
            <button
              className="button secondary"
              disabled={page <= 1}
              onClick={() => setPage((p) => p - 1)}
            >
              <ArrowLeft size={16} />
              Previous
            </button>
            <button
              className="button secondary"
              disabled={page >= posts.data.pages}
              onClick={() => setPage((p) => p + 1)}
            >
              Next
              <ArrowRight size={16} />
            </button>
          </div>
        </>
      )}
      {view === 'trending' && <Trending serverId={serverId} />}
      {editingArtist && (
        <ArtistProfile
          artist={editingArtist}
          categories={categoryRows.map((r) => r.category)}
          onClose={() => setEditingArtist(null)}
          save={(value) => act(`/artists/${editingArtist.id}/profile`, 'PUT', value)}
        />
      )}
      {artistView && manager && (
        <FeedTransfer
          serverId={serverId}
          onImported={() => void client.invalidateQueries({ queryKey: ['community', serverId] })}
        />
      )}
      {artistView && artists.data && (
        <>
          {bulkStatus && (
            <p className="notice" role="status">
              {bulkStatus}
            </p>
          )}
          {bulkMode && manager && (
            <BulkCategorize
              serverId={serverId}
              artists={artists.data}
              visibleArtists={filteredArtists}
              categories={categoryRows.map((r) => r.category)}
              selected={selectedArtists}
              onSelectedChange={setSelectedArtists}
              onApplied={(message) => {
                setBulkStatus(message);
                setBulkMode(false);
              }}
              onClose={() => {
                setBulkMode(false);
                setSelectedArtists(new Set());
              }}
            />
          )}
          <div className="community-artists">
            {filteredArtists.map((a) => (
              <article
                className={`community-artist${bulkMode && selectedArtists.has(a.id) ? ' selected' : ''}`}
                key={a.id}
              >
                {bulkMode && manager && (
                  <input
                    type="checkbox"
                    className="community-artist-select"
                    aria-label={`Select ${a.name}`}
                    checked={selectedArtists.has(a.id)}
                    onChange={(e) => {
                      const next = new Set(selectedArtists);
                      if (e.target.checked) next.add(a.id);
                      else next.delete(a.id);
                      setSelectedArtists(next);
                    }}
                  />
                )}
                <div className="community-artist-avatar">
                  {a.avatar_url ? (
                    <img src={a.avatar_url} alt={a.name} referrerPolicy="no-referrer" />
                  ) : (
                    a.name.slice(0, 1).toUpperCase()
                  )}
                </div>
                <div className="community-artist-info">
                  <h2>{a.name}</h2>
                  <span>{a.source}</span>
                  <TagBadges category={a.category || 'Uncategorized'} />
                  <small>ID: {a.id}</small>
                  <div className="artist-official-links">
                    {a.official_links?.map((link, i) => (
                      <a key={i} href={link.url} target="_blank" rel="noreferrer">
                        {link.label}
                        <ExternalLink size={12} />
                      </a>
                    ))}
                  </div>
                  {a.last_error && (
                    <p className="error">
                      Feed sync failed. An administrator can review this connection.
                    </p>
                  )}
                </div>
                <div className="community-artist-actions">
                  {manager && (
                    <>
                      <button
                        className="button secondary"
                        disabled={busy}
                        onClick={() => void act(`/artists/${a.id}/sync`, 'POST')}
                      >
                        Sync feed
                      </button>
                      <button
                        className="button secondary"
                        disabled={busy}
                        onClick={() =>
                          void act(`/artists/${a.id}/delivery`, 'PUT', {
                            enabled: a.enabled === false,
                          })
                        }
                      >
                        {a.enabled === false ? 'Resume announcements' : 'Pause announcements'}
                      </button>
                    </>
                  )}

                  {manager && (
                    <button className="button secondary" onClick={() => setEditingArtist(a)}>
                      <Pencil size={15} />
                      Edit profile
                    </button>
                  )}
                  <button
                    className={a.favorite ? 'button selected' : 'button secondary'}
                    disabled={busy}
                    aria-pressed={a.favorite}
                    onClick={() => void act(`/artists/${a.id}`, 'PATCH', { favorite: !a.favorite })}
                  >
                    <Bookmark size={16} />
                    {a.favorite ? 'Favorited' : 'Favorite'}
                  </button>
                  <button
                    className={a.notify ? 'button selected' : 'button secondary'}
                    disabled={busy}
                    aria-pressed={a.notify}
                    onClick={() => void act(`/artists/${a.id}`, 'PATCH', { notify: !a.notify })}
                  >
                    <Bell size={16} />
                    {a.notify ? 'Mentions on' : 'Notify me'}
                  </button>
                  {manager && (
                    <button
                      className="button secondary"
                      disabled={busy}
                      onClick={() => {
                        if (
                          confirm(
                            'Stop tracking this artist for this server? Artist favorites and mention preferences will be removed.',
                          )
                        )
                          void act(`/artists/${a.id}`, 'DELETE');
                      }}
                    >
                      Untrack
                    </button>
                  )}
                </div>
              </article>
            ))}
          </div>
          {!filteredArtists.length && (
            <div className="community-empty">
              <Users size={30} />
              <h2>
                {category !== '*' || artistSearch ? 'No matching artists.' : 'No artists yet.'}
              </h2>
              <p>
                {category !== '*' || artistSearch
                  ? 'Choose another category or clear the filter.'
                  : view === 'favorite-artists'
                    ? 'Favorite an artist to keep them here.'
                    : 'A server administrator can add the first artist.'}
              </p>
            </div>
          )}
          <p className="hint">
            “Notify me” opts you into Discord mentions for future posts. Favoriting an artist never
            enables mentions. Server settings may pause notifications.
          </p>
        </>
      )}
      {view === 'settings' && manager && server.data && (
        <ServerSettings
          key={JSON.stringify(server.data.server)}
          server={server.data.server}
          busy={busy}
          save={(data) => act('', 'PUT', data)}
        />
      )}
      {view === 'settings' && manager && <Categories serverId={serverId} />}
      {view === 'settings' && manager && <WebhookManager serverId={serverId} />}
      {view === 'activity' && manager && <FeedHealth serverId={serverId} />}
      {view === 'activity' && manager && <AiTagActivity serverId={serverId} />}
      {view === 'activity' && manager && <WebhookActivity serverId={serverId} />}
      {view === 'activity' && manager && (
        <div className="community-activity">
          {activity.data?.map((d) => (
            <article key={d.id}>
              <div>
                <strong>{d.title}</strong>
                <p>
                  {d.artist} · {date(d.created_at)}
                </p>
                {d.last_error && <p className="error">{d.last_error}</p>}
              </div>
              <span>
                {d.status} · {d.attempts} attempts
              </span>
              {d.status === 'failed' && (
                <button
                  className="button secondary"
                  disabled={busy}
                  onClick={() => void act(`/deliveries/${d.id}/retry`, 'POST')}
                >
                  Retry
                </button>
              )}
            </article>
          ))}
          {activity.data?.length === 0 && (
            <p className="notice">
              No bot deliveries yet. Initial feed history is imported silently.
            </p>
          )}
        </div>
      )}
      {adding && (
        <FeedForm
          categories={categoryRows.map((r) => r.category)}
          error={error}
          onClose={() => setAdding(false)}
          onSubmit={async (data) => {
            const ok = await act('/artists', 'POST', data);
            if (ok) setAdding(false);
            return ok;
          }}
        />
      )}
    </>
  );
}
function PostCard({
  post: p,
  serverId,
  manager,
  busy,
  change,
}: {
  post: Post;
  serverId: string;
  manager: boolean;
  busy: boolean;
  change: (value: { liked?: boolean; favorite?: boolean }) => void;
}) {
  const image = p.media.find((m) => m.kind === 'image');
  return (
    <article className="community-post">
      <a className="community-post-image" href={postUrl(serverId, p.id)}>
        {image ? (
          <img src={image.url} loading="lazy" referrerPolicy="no-referrer" alt={p.title} />
        ) : (
          <ImageOff size={36} />
        )}
      </a>
      <div className="community-post-body">
        <span className="eyebrow">{p.feed_name}</span>
        <h2>
          <a href={postUrl(serverId, p.id)}>{p.title}</a>
        </h2>
        <p>
          {p.author} · {date(p.published_at)}
        </p>
        <TagBadges tags={p.tags?.slice(0, 5)} category={p.category} />
        <p className="community-summary">{p.summary}</p>
        <div className="post-card-tools">
          <PostComments serverId={serverId} post={p} />
          <PostFiles serverId={serverId} post={p} manager={manager} />
        </div>
        <div className="community-post-actions">
          <button
            disabled={busy}
            aria-label={p.liked ? 'Unlike post' : 'Like post'}
            aria-pressed={p.liked}
            onClick={() => change({ liked: !p.liked })}
          >
            <Heart size={18} fill={p.liked ? 'currentColor' : 'none'} />
            <span>{p.likes}</span>
          </button>
          <button
            disabled={busy}
            aria-label={p.favorite ? 'Unsave post' : 'Save post'}
            aria-pressed={p.favorite}
            onClick={() => change({ favorite: !p.favorite })}
          >
            <Bookmark size={18} fill={p.favorite ? 'currentColor' : 'none'} />
          </button>
          <a href={p.url} target="_blank" rel="noreferrer" aria-label="Open original post">
            <ExternalLink size={18} />
          </a>
        </div>
      </div>
    </article>
  );
}
function ServerSettings({
  server,
  busy,
  save,
}: {
  server: Server;
  busy: boolean;
  save: (data: unknown) => Promise<boolean>;
}) {
  const [channel, setChannel] = useState(server.channel_id || '');
  const [enabled, setEnabled] = useState(server.enabled);
  const [mentions, setMentions] = useState(server.mentions_enabled);
  const [aiTags, setAiTags] = useState(server.ai_tags_enabled ?? false);
  const [saved, setSaved] = useState(false);
  return (
    <form
      className="community-settings"
      onSubmit={async (e) => {
        e.preventDefault();
        setSaved(
          await save({
            channel_id: channel,
            enabled,
            mentions_enabled: mentions,
            ai_tags_enabled: aiTags,
          }),
        );
      }}
    >
      <h2>Announcements</h2>
      <p>
        New artwork goes to{' '}
        {server.channel_name ? '#' + server.channel_name : 'your configured channel'} as an embed
        with personal action buttons.
      </p>
      <label>
        Discord channel ID
        <input
          pattern="[0-9]{1,20}"
          value={channel}
          onChange={(e) => setChannel(e.target.value)}
          placeholder="Copy the channel ID from Discord"
        />
      </label>
      <p className="hint">
        Enable Developer Mode in Discord to copy IDs. The bot needs View Channel, Send Messages and
        Embed Links in this channel. Leave blank to use category channels and webhooks only.
      </p>
      <Toggle
        checked={enabled}
        onChange={setEnabled}
        label="Send new posts"
        description="Pause bot and webhook deliveries without losing queued posts."
      />
      <Toggle
        checked={mentions}
        onChange={setMentions}
        label="Allow artist mentions"
        description="Only users who explicitly opted in will be mentioned."
      />
      <Toggle
        checked={aiTags}
        onChange={setAiTags}
        label="Automatically tag new posts"
        description="Analyze up to two images per new post with the configured WD tagger. Tags are saved automatically before delivery when ready; you can edit them later. Posts without images are skipped."
      />
      <div className="notice">
        The website library is visible to all current server members, independently of the
        announcement channel’s permissions. Favorites remain private.
      </div>
      <button className="button primary" disabled={busy}>
        Save settings
      </button>
      {saved && <span role="status">Settings saved.</span>}
    </form>
  );
}
