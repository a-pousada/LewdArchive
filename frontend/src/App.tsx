import Login from './features/Login';
import EntryCard from './features/EntryCard';
import FeedForm from './features/FeedForm';
import SettingsPanel from './features/SettingsPanel';
import EntryDetail from './features/EntryDetail';
import { useEffect, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Archive,
  ArrowDownToLine,
  Bookmark,
  CheckCheck,
  ChevronLeft,
  ChevronRight,
  Clock3,
  Grid2X2,
  ImageOff,
  LayoutList,
  LoaderCircle,
  LogOut,
  Menu,
  Plus,
  Radio,
  RefreshCw,
  Rss,
  Search,
  Settings2,
  SlidersHorizontal,
  Sparkles,
  Trash2,
  Volume2,
  X,
  Activity,
  Pause,
  Play,
  Pencil,
  AlertCircle,
} from 'lucide-react';
import { api, bytes, date, sources } from './api';
import type { Feed, Entry, Stats, Settings, Job, Download } from './api';
import { Logo, Source, Modal, Empty } from './components';
type View = 'library' | 'unread' | 'favorites' | 'feeds' | 'downloads' | 'activity' | 'settings';
const titles: Record<View, string> = {
  library: 'Your library',
  unread: 'New posts',
  favorites: 'Favorites',
  feeds: 'Your feeds',
  downloads: 'Downloads',
  activity: 'Activity',
  settings: 'Settings',
};
const subtitles: Record<View, string> = {
  library: 'Follow artists. Keep your discoveries.',
  unread: 'The latest posts, ready to explore.',
  favorites: 'A place for the artwork you want to keep.',
  feeds: 'Stay connected to your favorite corners of the internet.',
  downloads: 'Your files, saved to your collection.',
  activity: 'Follow every notification and download.',
  settings: 'Your archive, your way.',
};
export default function App() {
  const [session, setSession] = useState<boolean | null>(null);
  const client = useQueryClient();
  useEffect(() => {
    api('/session')
      .then(() => setSession(true))
      .catch(() => setSession(false));
    const expire = () => {
      setSession(false);
      client.clear();
    };
    window.addEventListener('session-expired', expire);
    return () => window.removeEventListener('session-expired', expire);
  }, [client]);
  if (session === null)
    return (
      <div className="boot">
        <Logo />
        <LoaderCircle className="spin" />
      </div>
    );
  return session ? (
    <Workspace
      onLogout={async () => {
        await api('/logout', 'POST');
        client.clear();
        setSession(false);
      }}
    />
  ) : (
    <Login onLogin={() => setSession(true)} />
  );
}
function Workspace({ onLogout }: { onLogout: () => Promise<void> }) {
  const [view, setView] = useState<View>('library');
  const [source, setSource] = useState('');
  const [search, setSearch] = useState('');
  const [debounced, setDebounced] = useState('');
  const [feedId, setFeedId] = useState('');
  const [page, setPage] = useState(1);
  const [layout, setLayout] = useState('grid');
  const [mobile, setMobile] = useState(false);
  const [feedModal, setFeedModal] = useState<Feed | 'new' | null>(null);
  const [selected, setSelected] = useState<Entry | null>(null);
  const [deleteFeed, setDeleteFeed] = useState<Feed | null>(null);
  const [toast, setToast] = useState('');
  const [busy, setBusy] = useState(false);
  const client = useQueryClient();
  const stats = useQuery({
    queryKey: ['stats'],
    queryFn: () => api<Stats>('/stats'),
    refetchInterval: 15000,
  });
  const feeds = useQuery({
    queryKey: ['feeds'],
    queryFn: () => api<Feed[]>('/feeds'),
    refetchInterval: 15000,
  });
  const settings = useQuery({ queryKey: ['settings'], queryFn: () => api<Settings>('/settings') });
  const library = ['library', 'unread', 'favorites'].includes(view);
  useEffect(() => {
    const t = setTimeout(() => setDebounced(search), 250);
    return () => clearTimeout(t);
  }, [search]);
  useEffect(() => {
    setPage(1);
  }, [source, debounced, feedId, view]);
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(''), 5000);
    return () => clearTimeout(t);
  }, [toast]);
  const params = new URLSearchParams({
    page: String(page),
    ...(source ? { source } : {}),
    ...(debounced ? { q: debounced } : {}),
    ...(feedId ? { feed_id: feedId } : {}),
    ...(view === 'unread' ? { unread: 'true' } : {}),
    ...(view === 'favorites' ? { favorite: 'true' } : {}),
  });
  const entries = useQuery({
    queryKey: ['entries', params.toString()],
    queryFn: () => api<{ items: Entry[]; total: number; pages: number }>(`/entries?${params}`),
    enabled: library,
    refetchInterval: 15000,
  });
  const jobs = useQuery({
    queryKey: ['jobs'],
    queryFn: () => api<Job[]>('/jobs'),
    enabled: view === 'activity',
    refetchInterval: 10000,
  });
  const downloads = useQuery({
    queryKey: ['downloads'],
    queryFn: () => api<Download[]>('/downloads'),
    enabled: view === 'downloads',
    refetchInterval: 15000,
  });
  const refresh = () => {
    void client.invalidateQueries();
  };
  const act = async (path: string, method = 'POST', body?: unknown, message = 'Changes saved.') => {
    setBusy(true);
    try {
      await api(path, method, body);
      refresh();
      setToast(message);
      return true;
    } catch (e) {
      setToast((e as Error).message);
      return false;
    } finally {
      setBusy(false);
    }
  };
  const navigate = (v: View) => {
    setView(v);
    setMobile(false);
    setSource('');
    setFeedId('');
    setSearch('');
  };
  const counts = stats.data;
  const nav = [
    { id: 'library' as View, icon: Archive, label: 'Library', count: counts?.entries },
    { id: 'unread' as View, icon: Sparkles, label: 'New posts', count: counts?.unread },
    { id: 'favorites' as View, icon: Bookmark, label: 'Favorites', count: counts?.favorites },
  ];
  const failed = (feeds.data || []).filter((f) => f.last_error).length;
  return (
    <div className="app-shell">
      {mobile && (
        <button
          className="sidebar-scrim"
          aria-label="Close menu"
          onClick={() => setMobile(false)}
        />
      )}
      <aside className={`sidebar ${mobile ? 'open' : ''}`}>
        <Logo />
        <div className="workspace-tag">
          <span className="workspace-avatar">LA</span>
          <span>
            My archive<small>Personal workspace</small>
          </span>
          <span className="local-tag">LOCAL</span>
        </div>
        <span className="nav-label">COLLECTION</span>
        <nav>
          {nav.map((n) => (
            <button
              key={n.id}
              className={view === n.id ? 'active' : ''}
              onClick={() => navigate(n.id)}
            >
              <n.icon size={18} />
              <span>{n.label}</span>
              {n.count !== undefined && <span className="nav-count">{n.count}</span>}
            </button>
          ))}
        </nav>
        <span className="nav-label">MANAGE</span>
        <nav>
          {[
            { id: 'feeds' as View, icon: Rss, label: 'Feeds' },
            { id: 'downloads' as View, icon: ArrowDownToLine, label: 'Downloads' },
            { id: 'activity' as View, icon: Activity, label: 'Activity' },
          ].map((n) => (
            <button
              key={n.id}
              className={view === n.id ? 'active' : ''}
              onClick={() => navigate(n.id)}
            >
              <n.icon size={18} />
              <span>{n.label}</span>
              {n.id === 'feeds' && <span className="nav-count">{counts?.feeds || 0}</span>}
              {n.id === 'activity' && !!counts?.failed_jobs && <span className="error-dot" />}
            </button>
          ))}
        </nav>
        <div className="sidebar-feeds">
          <div className="nav-label">
            YOUR CONNECTIONS
            <button aria-label="Add feed" onClick={() => setFeedModal('new')}>
              <Plus size={14} />
            </button>
          </div>
          {(feeds.data || []).slice(0, 5).map((f) => (
            <button
              className="mini-feed"
              key={f.id}
              onClick={() => {
                navigate('library');
                setFeedId(f.id);
              }}
            >
              <span className={`feed-dot ${f.last_error ? 'failed' : f.enabled ? '' : 'paused'}`} />
              <span>{f.name}</span>
            </button>
          ))}
          {!feeds.data?.length && (
            <p>
              Your favorite artists
              <br />
              will appear here.
            </p>
          )}
        </div>
        <div className="sidebar-bottom">
          <div className="monitor-status">
            <span className={`feed-dot ${stats.isError ? 'failed' : ''}`} />
            <span>
              {stats.isError ? 'Server unavailable' : 'Observatory online'}
              <small>{counts?.active_feeds || 0} active feeds · automatic checks</small>
            </span>
            <Radio size={15} />
          </div>
          <button
            className={view === 'settings' ? 'active' : ''}
            onClick={() => navigate('settings')}
          >
            <Settings2 size={18} />
            Settings
          </button>
          <button onClick={() => void onLogout().catch((e) => setToast(e.message))}>
            <LogOut size={17} />
            Sign out
          </button>
          <div className="sidebar-signature">
            <span>LEWDARCHIVE</span>
            <span>v0.1 / NIGHT</span>
          </div>
        </div>
      </aside>
      <main>
        <header className="topbar">
          <div className="breadcrumb">
            <button
              className="icon-button mobile-menu"
              aria-label="Open menu"
              onClick={() => setMobile(true)}
            >
              <Menu size={20} />
            </button>
            <span>My archive</span>
            <ChevronRight size={13} />
            <strong>{titles[view]}</strong>
          </div>
          <div className="topbar-right">
            <span className="system-pill">
              <span className="feed-dot" />
              PERSONAL ARCHIVE
            </span>
            <span className="avatar">LA</span>
          </div>
        </header>
        <div className="main-content">
          <div className="page-heading">
            <div>
              <span className="eyebrow">
                {view === 'library' ? 'THE COLLECTION / YOUR ARCHIVE' : 'LEWDARCHIVE / YOUR SPACE'}
              </span>
              <h1>
                {titles[view]}
                <span className="heading-dot">.</span>
              </h1>
              <p>{subtitles[view]}</p>
            </div>
            <div className="heading-actions">
              <button
                className="button secondary"
                disabled={busy}
                onClick={() =>
                  void act(
                    '/sync',
                    'POST',
                    undefined,
                    'Sync scheduled. Posts will update automatically.',
                  )
                }
              >
                <RefreshCw size={15} className={busy ? 'spin' : ''} />
                <span>Sync</span>
              </button>
              <button className="button primary" onClick={() => setFeedModal('new')}>
                <Plus size={17} />
                New feed
              </button>
            </div>
          </div>
          {(view === 'library' || view === 'feeds') && (
            <div className="overview">
              <div>
                <span className="stat-icon">
                  <Rss size={17} />
                </span>
                <span>
                  <small>Followed feeds</small>
                  <strong>
                    {counts?.feeds ?? '—'}
                    <em>{counts?.active_feeds || 0} active</em>
                  </strong>
                </span>
              </div>
              <div>
                <span className="stat-icon">
                  <ImageOff size={17} />
                </span>
                <span>
                  <small>Archived posts</small>
                  <strong>
                    {counts?.entries ?? '—'}
                    <em>in your collection</em>
                  </strong>
                </span>
              </div>
              <div>
                <span className="stat-icon pink">
                  <Sparkles size={17} />
                </span>
                <span>
                  <small>New posts</small>
                  <strong>
                    {counts?.unread ?? '—'}
                    <em>to discover</em>
                  </strong>
                </span>
              </div>
              <div>
                <span className="stat-icon">
                  <ArrowDownToLine size={17} />
                </span>
                <span>
                  <small>Saved files</small>
                  <strong>
                    {counts?.downloads ?? '—'}
                    <em>{bytes(counts?.storage_bytes || 0)}</em>
                  </strong>
                </span>
              </div>
            </div>
          )}
          {!!failed && view === 'feeds' && (
            <div className="notice">
              <AlertCircle size={17} />
              {failed} feed(s) need attention. See the error messages below.
            </div>
          )}
          {library && (
            <>
              <section className="collection-toolbar">
                <div className="source-tabs">
                  <button className={!source ? 'selected' : ''} onClick={() => setSource('')}>
                    All sources
                  </button>
                  {sources
                    .filter((s) => s.id !== 'rss')
                    .map((s) => (
                      <button
                        key={s.id}
                        className={source === s.id ? 'selected' : ''}
                        onClick={() => setSource(s.id)}
                      >
                        <span>{s.mark}</span>
                        {s.label}
                      </button>
                    ))}
                </div>
                <div className="filter-line">
                  <label className="search-input">
                    <Search size={17} />
                    <input
                      aria-label="Search posts or artists"
                      placeholder="Search posts or artists..."
                      value={search}
                      onChange={(e) => setSearch(e.target.value)}
                    />
                    {search && (
                      <button aria-label="Clear search" onClick={() => setSearch('')}>
                        <X size={14} />
                      </button>
                    )}
                  </label>
                  <div className="filter-right">
                    <label className="feed-filter">
                      <SlidersHorizontal size={15} />
                      <select
                        aria-label="Filter by feed"
                        value={feedId}
                        onChange={(e) => setFeedId(e.target.value)}
                      >
                        <option value="">All feeds</option>
                        {feeds.data?.map((f) => (
                          <option value={f.id} key={f.id}>
                            {f.name}
                          </option>
                        ))}
                      </select>
                    </label>
                    <div className="view-toggle">
                      <button
                        className={layout === 'grid' ? 'selected' : ''}
                        aria-label="Grid view"
                        aria-pressed={layout === 'grid'}
                        onClick={() => setLayout('grid')}
                      >
                        <Grid2X2 size={16} />
                      </button>
                      <button
                        className={layout === 'list' ? 'selected' : ''}
                        aria-label="List view"
                        aria-pressed={layout === 'list'}
                        onClick={() => setLayout('list')}
                      >
                        <LayoutList size={17} />
                      </button>
                    </div>
                  </div>
                </div>
              </section>
              <div className="result-line">
                <span>
                  {entries.data?.total || 0} posts <span className="muted">/ newest first</span>
                </span>
                {!!counts?.unread && (
                  <button
                    disabled={busy}
                    onClick={() =>
                      void act('/entries/read-all', 'POST', undefined, 'All posts marked as read.')
                    }
                  >
                    <CheckCheck size={15} />
                    Mark all as read
                  </button>
                )}
              </div>
              {entries.isError ? (
                <ErrorPanel error={entries.error} retry={() => void entries.refetch()} />
              ) : entries.isPending ? (
                <Loading />
              ) : entries.data?.items.length ? (
                <>
                  <div className={`entry-grid ${layout === 'list' ? 'list-layout' : ''}`}>
                    {entries.data.items.map((e) => (
                      <EntryCard
                        key={e.id}
                        entry={e}
                        onOpen={() => {
                          setSelected(e);
                          if (!e.is_read)
                            void act(
                              `/entries/${e.id}`,
                              'PATCH',
                              { is_read: true },
                              'Post marked as read.',
                            );
                        }}
                        onFavorite={() =>
                          void act(
                            `/entries/${e.id}`,
                            'PATCH',
                            { favorite: !e.favorite },
                            e.favorite ? 'Removed from favorites.' : 'Saved to favorites.',
                          )
                        }
                      />
                    ))}
                  </div>
                  <div className="pagination">
                    <button
                      className="button secondary"
                      disabled={page === 1}
                      onClick={() => setPage((p) => p - 1)}
                    >
                      <ChevronLeft size={16} />
                      Previous
                    </button>
                    <span>
                      Page {page} of {entries.data.pages}
                    </span>
                    <button
                      className="button secondary"
                      disabled={page >= entries.data.pages}
                      onClick={() => setPage((p) => p + 1)}
                    >
                      Next
                      <ChevronRight size={16} />
                    </button>
                  </div>
                </>
              ) : (
                <Empty
                  kind={
                    search || source || feedId || view === 'unread'
                      ? 'search'
                      : view === 'favorites'
                        ? 'favorites'
                        : 'library'
                  }
                  onAdd={!counts?.feeds ? () => setFeedModal('new') : undefined}
                />
              )}
            </>
          )}
          {view === 'feeds' &&
            (feeds.isError ? (
              <ErrorPanel error={feeds.error} retry={() => void feeds.refetch()} />
            ) : feeds.isPending ? (
              <Loading />
            ) : feeds.data?.length ? (
              <div className="feed-list">
                {feeds.data.map((f) => (
                  <article className="feed-card" key={f.id}>
                    <div className={`feed-monogram source-${f.source}`}>
                      {f.name.slice(0, 2).toUpperCase()}
                    </div>
                    <div className="feed-info">
                      <div>
                        <h3>{f.name}</h3>
                        <Source id={f.source} />
                        <span
                          className={`badge ${f.last_error ? 'danger' : f.enabled ? 'good' : ''}`}
                        >
                          {f.last_error ? 'Needs attention' : f.enabled ? 'Active' : 'Paused'}
                        </span>
                      </div>
                      <p className="feed-url">{f.url}</p>
                      <div className="feed-meta">
                        <span>
                          <Clock3 size={13} />
                          Every {f.poll_minutes} min
                        </span>
                        <span>
                          <Volume2 size={13} />
                          {f.notify ? 'Discord enabled' : 'Notifications off'}
                        </span>
                        <span>
                          <ArrowDownToLine size={13} />
                          {f.auto_download ? 'Automatic downloads' : 'Manual downloads'}
                        </span>
                      </div>
                      {f.last_error && <p className="error">{f.last_error}</p>}
                      <small>Last checked: {date(f.last_checked_at)}</small>
                    </div>
                    <div className="feed-actions">
                      <button
                        className="icon-button"
                        aria-label={`Sync ${f.name}`}
                        disabled={!f.enabled || busy}
                        onClick={() => void act(`/feeds/${f.id}/sync`)}
                      >
                        <RefreshCw size={16} />
                      </button>
                      <button
                        className="icon-button"
                        aria-label={f.enabled ? `Pause ${f.name}` : `Enable ${f.name}`}
                        disabled={busy}
                        onClick={() =>
                          void act(`/feeds/${f.id}`, 'PUT', { ...f, enabled: !f.enabled })
                        }
                      >
                        {f.enabled ? <Pause size={16} /> : <Play size={16} />}
                      </button>
                      <button
                        className="icon-button"
                        aria-label={`Edit ${f.name}`}
                        onClick={() => setFeedModal(f)}
                      >
                        <Pencil size={16} />
                      </button>
                      <button
                        className="icon-button danger-text"
                        aria-label={`Delete ${f.name}`}
                        onClick={() => setDeleteFeed(f)}
                      >
                        <Trash2 size={16} />
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            ) : (
              <Empty kind="feeds" onAdd={() => setFeedModal('new')} />
            ))}
          {view === 'settings' &&
            (settings.isError ? (
              <ErrorPanel error={settings.error} retry={() => void settings.refetch()} />
            ) : settings.data ? (
              <SettingsPanel
                initial={settings.data}
                onSave={async (body) => act('/settings', 'PUT', body, 'Settings saved.')}
              />
            ) : (
              <Loading />
            ))}
          {view === 'activity' && (
            <>
              <div className="section-note">
                <span>
                  <Activity size={16} />
                  Processing queue
                </span>
                <small>Latest 100 jobs · updated automatically</small>
              </div>
              {jobs.isError ? (
                <ErrorPanel error={jobs.error} retry={() => void jobs.refetch()} />
              ) : jobs.isPending ? (
                <Loading />
              ) : jobs.data?.length ? (
                <div className="table-wrap">
                  <table>
                    <thead>
                      <tr>
                        <th>Post</th>
                        <th>Action</th>
                        <th>Status</th>
                        <th>Attempts</th>
                        <th>Date</th>
                        <th />
                      </tr>
                    </thead>
                    <tbody>
                      {jobs.data.map((j) => (
                        <tr key={j.id}>
                          <td>
                            <strong>{j.title}</strong>
                            <small>{j.feed_name}</small>
                            {j.last_error && <small className="error">{j.last_error}</small>}
                          </td>
                          <td>
                            <span className="inline-icon">
                              {j.kind === 'discord' ? (
                                <Volume2 size={15} />
                              ) : (
                                <ArrowDownToLine size={15} />
                              )}{' '}
                              {j.kind === 'discord' ? 'Discord' : 'Download'}
                            </span>
                          </td>
                          <td>
                            <span
                              className={`badge ${j.status === 'done' ? 'good' : j.status === 'failed' ? 'danger' : ''}`}
                            >
                              {
                                (
                                  {
                                    done: 'Completed',
                                    failed: 'Failed',
                                    pending: 'Queued',
                                    processing: 'Processing',
                                  } as Record<string, string>
                                )[j.status]
                              }
                            </span>
                          </td>
                          <td>{j.attempts}/8</td>
                          <td>{date(j.created_at)}</td>
                          <td>
                            {j.status === 'failed' && (
                              <button
                                className="icon-button"
                                aria-label="Try again"
                                disabled={busy}
                                onClick={() => void act(`/jobs/${j.id}/retry`)}
                              >
                                <RefreshCw size={16} />
                              </button>
                            )}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <Empty kind="activity" />
              )}
            </>
          )}
          {view === 'downloads' && (
            <>
              <div className="section-note">
                <span>
                  <ArrowDownToLine size={16} />
                  {counts?.downloads || 0} files · {bytes(counts?.storage_bytes || 0)}
                </span>
                <small>Latest 200 saved files</small>
              </div>
              {downloads.isError ? (
                <ErrorPanel error={downloads.error} retry={() => void downloads.refetch()} />
              ) : downloads.isPending ? (
                <Loading />
              ) : downloads.data?.length ? (
                <div className="table-wrap">
                  <table>
                    <thead>
                      <tr>
                        <th>File</th>
                        <th>Format</th>
                        <th>Size</th>
                        <th>Saved on</th>
                        <th />
                      </tr>
                    </thead>
                    <tbody>
                      {downloads.data.map((d) => (
                        <tr key={d.id}>
                          <td>
                            <strong>{d.title}</strong>
                            <small>{d.author}</small>
                          </td>
                          <td>
                            <span className="badge">{d.content_type.split('/')[1]}</span>
                          </td>
                          <td>{bytes(d.size_bytes)}</td>
                          <td>{date(d.created_at)}</td>
                          <td>
                            <a
                              className="button secondary"
                              href={`/api/downloads/${d.id}/file`}
                              download
                            >
                              <ArrowDownToLine size={15} />
                              Download
                            </a>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <Empty kind="downloads" />
              )}
            </>
          )}
          <footer className="content-footer">
            <span>Your own little universe of artists.</span>
            <span>
              COLLECT WITH CARE. <span className="pink-text">KEEP ART.</span>
            </span>
          </footer>
        </div>
      </main>
      {toast && (
        <div className="toast" role="status">
          <span>{toast}</span>
          <button aria-label="Dismiss message" onClick={() => setToast('')}>
            <X size={16} />
          </button>
        </div>
      )}
      {feedModal && (
        <FeedForm
          feed={feedModal === 'new' ? undefined : feedModal}
          onClose={() => setFeedModal(null)}
          onSubmit={async (data) => {
            const success = await act(
              feedModal === 'new' ? '/feeds' : `/feeds/${feedModal.id}`,
              feedModal === 'new' ? 'POST' : 'PUT',
              data,
              feedModal === 'new'
                ? 'Feed added. Importing history without sending old notifications.'
                : 'Feed updated.',
            );
            if (success) setFeedModal(null);
            return success;
          }}
        />
      )}
      {selected && (
        <EntryDetail
          entry={selected}
          onSendDiscord={async () => {
            try {
              await api(`/entries/${selected.id}/discord`, 'POST');
              refresh();
              return null;
            } catch (error) {
              return (error as Error).message;
            }
          }}
          onClose={() => setSelected(null)}
          onDownload={() =>
            void act(`/entries/${selected.id}/download`, 'POST', undefined, 'Download queued.')
          }
          onFavorite={async () => {
            if (await act(`/entries/${selected.id}`, 'PATCH', { favorite: !selected.favorite }))
              setSelected({ ...selected, favorite: !selected.favorite });
          }}
        />
      )}
      {deleteFeed && (
        <Modal
          title="Remove this feed?"
          subtitle={deleteFeed.name}
          onClose={() => setDeleteFeed(null)}
        >
          <p className="modal-copy">
            This will remove the feed, its posts, favorites, and job records. Files already saved on
            disk will remain, but will no longer appear in the interface.
          </p>
          <div className="modal-actions">
            <button className="button secondary" onClick={() => setDeleteFeed(null)}>
              Cancel
            </button>
            <button
              className="button destructive"
              disabled={busy}
              onClick={async () => {
                if (await act(`/feeds/${deleteFeed.id}`, 'DELETE', undefined, 'Feed removed.'))
                  setDeleteFeed(null);
              }}
            >
              <Trash2 size={16} />
              Remove feed
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
function Loading() {
  return (
    <div className="loading">
      <LoaderCircle className="spin" size={24} />
      <span>Opening your archive...</span>
    </div>
  );
}
function ErrorPanel({ error, retry }: { error: Error | null; retry: () => void }) {
  return (
    <div className="error-panel">
      <AlertCircle size={24} />
      <h3>Unable to load</h3>
      <p>{error?.message}</p>
      <button className="button secondary" onClick={retry}>
        <RefreshCw size={15} />
        Try again
      </button>
    </div>
  );
}
