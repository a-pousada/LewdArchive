import { useEffect, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Activity,
  ArrowUpRight,
  CheckCircle2,
  Database,
  HardDrive,
  LayoutDashboard,
  LogOut,
  Menu,
  Server,
  Settings2,
  ShieldCheck,
  Users,
  X,
} from 'lucide-react';
import { api, bytes, date } from './api';
import { Logo, Toggle } from './components';
import Login from './features/Login';
interface Overview {
  servers: number;
  users: number;
  subscriptions: number;
  posts: number;
  comments: number;
  storage_bytes: number;
  failed: number;
  pending: number;
  feed_errors: number;
}
interface ServerRow {
  id: string;
  name: string;
  platform: string;
  enabled: boolean;
  channel_name: string;
  artists: number;
  webhooks: number;
  created_at: string;
}
interface QueueRow {
  id: string;
  kind: string;
  server: string;
  status: string;
  attempts: number;
  last_error: string | null;
  created_at: string;
}
interface Services {
  database: string;
  discord: string;
  rsshub: string;
  last_poll: string | null;
  max_download_mb: number;
}
interface Settings {
  downloads_enabled: boolean;
  notifications_enabled: boolean;
}
const sections = [
  { id: 'overview', label: 'Overview', icon: LayoutDashboard },
  { id: 'servers', label: 'Servers', icon: Users },
  { id: 'queues', label: 'Queues', icon: Activity },
  { id: 'services', label: 'Services', icon: Server },
  { id: 'settings', label: 'Settings', icon: Settings2 },
] as const;
type Section = (typeof sections)[number]['id'];
export default function App() {
  const client = useQueryClient();
  const [session, setSession] = useState<boolean | null>(null);
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
        <p role="status">Connecting to administration…</p>
      </div>
    );
  if (!session) return <Login onLogin={() => setSession(true)} />;
  return (
    <Administration
      onLogout={async () => {
        await api('/logout', 'POST');
        client.clear();
        setSession(false);
      }}
    />
  );
}
function Administration({ onLogout }: { onLogout: () => Promise<void> }) {
  const [section, setSection] = useState<Section>('overview');
  const [mobile, setMobile] = useState(false);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const client = useQueryClient();
  const overview = useQuery({
    queryKey: ['admin', 'overview'],
    queryFn: () => api<Overview>('/admin/overview'),
    refetchInterval: 30000,
  });
  const servers = useQuery({
    queryKey: ['admin', 'servers'],
    queryFn: () => api<ServerRow[]>('/admin/servers'),
    enabled: section === 'servers',
  });
  const queues = useQuery({
    queryKey: ['admin', 'queues'],
    queryFn: () => api<QueueRow[]>('/admin/queues'),
    enabled: section === 'queues',
    refetchInterval: section === 'queues' ? 15000 : false,
  });
  const services = useQuery({
    queryKey: ['admin', 'services'],
    queryFn: () => api<Services>('/admin/services'),
    enabled: section === 'services' || section === 'overview',
    refetchInterval: 30000,
  });
  const settings = useQuery({
    queryKey: ['admin', 'settings'],
    queryFn: () => api<Settings>('/admin/settings'),
    enabled: section === 'settings',
  });
  async function act(path: string, body?: unknown) {
    setBusy(true);
    setError('');
    try {
      await api(path, body ? 'PUT' : 'POST', body);
      await client.invalidateQueries({ queryKey: ['admin'] });
      return true;
    } catch (e) {
      setError((e as Error).message);
      return false;
    } finally {
      setBusy(false);
    }
  }
  const query =
    section === 'overview'
      ? overview
      : section === 'servers'
        ? servers
        : section === 'queues'
          ? queues
          : section === 'services'
            ? services
            : settings;
  const description = {
    overview: 'A clear view of your bot and the communities it serves.',
    servers: 'Connected communities and their delivery configuration.',
    queues: 'Monitor delivery and media processing across the installation.',
    services: 'Configuration and operational signals from this installation.',
    settings: 'Installation-wide controls. Server preferences stay in Community.',
  }[section];
  return (
    <div className="admin-shell">
      <aside className={`admin-sidebar ${mobile ? 'is-open' : ''}`}>
        <Logo />
        <div className="admin-identity">
          <ShieldCheck size={21} />
          <div>
            <strong>Bot administration</strong>
            <small>Installation access</small>
          </div>
        </div>
        <nav aria-label="Administration">
          {sections.map((item) => (
            <button
              className={section === item.id ? 'active' : ''}
              key={item.id}
              onClick={() => {
                setSection(item.id);
                setMobile(false);
                setError('');
              }}
            >
              <item.icon size={18} />
              {item.label}
              {item.id === 'queues' && !!overview.data?.failed && (
                <span className="admin-nav-count">{overview.data.failed}</span>
              )}
            </button>
          ))}
        </nav>
        <div className="admin-sidebar-footer">
          <a href="/community">
            <Users size={17} />
            Open Community
            <ArrowUpRight size={14} />
          </a>
          <button onClick={() => void onLogout().catch((e) => setError(e.message))}>
            <LogOut size={17} />
            Sign out
          </button>
          <small>LEWDARCHIVE / ADMINISTRATION</small>
        </div>
      </aside>
      {mobile && (
        <button
          className="admin-backdrop"
          aria-label="Close navigation"
          onClick={() => setMobile(false)}
        />
      )}
      <main className="admin-main">
        <header className="admin-topbar">
          <button
            className="icon-button admin-menu"
            aria-label="Open navigation"
            onClick={() => setMobile(!mobile)}
          >
            {mobile ? <X size={18} /> : <Menu size={18} />}
          </button>
          <span>
            Administration <span>/</span> {sections.find((s) => s.id === section)?.label}
          </span>
          <span className="admin-access">
            <ShieldCheck size={14} />
            Administrator
          </span>
        </header>
        <div className="admin-content">
          <div className="admin-heading">
            <span className="eyebrow">LEWDARCHIVE / OPERATIONS</span>
            <h1>
              {section === 'overview'
                ? 'Installation overview'
                : sections.find((s) => s.id === section)?.label}
            </h1>
            <p>{description}</p>
          </div>
          {error && (
            <p className="error" role="alert">
              {error}
            </p>
          )}
          {query.isError && (
            <div className="notice" role="alert">
              {query.error.message}
              <button className="button secondary" onClick={() => void query.refetch()}>
                Try again
              </button>
            </div>
          )}
          {query.isPending && <p role="status">Loading {section}…</p>}
          {section === 'overview' && overview.data && (
            <>
              <div className="admin-metrics">
                {[
                  { label: 'Connected servers', value: overview.data.servers, icon: Users },
                  { label: 'Registered members', value: overview.data.users, icon: ShieldCheck },
                  { label: 'Pending jobs', value: overview.data.pending, icon: Activity },
                  {
                    label: 'Media storage',
                    value: bytes(overview.data.storage_bytes),
                    icon: HardDrive,
                  },
                ].map((m) => (
                  <article key={m.label}>
                    <m.icon size={21} />
                    <span>{m.label}</span>
                    <strong>{m.value}</strong>
                  </article>
                ))}
              </div>
              <div className="admin-panels">
                <section className="admin-panel">
                  <div className="admin-panel-title">
                    <Activity size={20} />
                    <h2>Attention & delivery</h2>
                  </div>
                  <dl>
                    <div>
                      <dt>Failed jobs</dt>
                      <dd className={overview.data.failed ? 'danger-text' : ''}>
                        {overview.data.failed}
                      </dd>
                    </div>
                    <div>
                      <dt>Connections with sync errors</dt>
                      <dd>{overview.data.feed_errors}</dd>
                    </div>
                    <div>
                      <dt>Last feed check</dt>
                      <dd>{date(services.data?.last_poll ?? null)}</dd>
                    </div>
                  </dl>
                  <button className="button secondary" onClick={() => setSection('queues')}>
                    Inspect queues
                    <ArrowUpRight size={15} />
                  </button>
                </section>
                <section className="admin-panel">
                  <div className="admin-panel-title">
                    <Database size={20} />
                    <h2>Community footprint</h2>
                  </div>
                  <dl>
                    <div>
                      <dt>Artist subscriptions</dt>
                      <dd>{overview.data.subscriptions}</dd>
                    </div>
                    <div>
                      <dt>Stored posts</dt>
                      <dd>{overview.data.posts}</dd>
                    </div>
                    <div>
                      <dt>Comments</dt>
                      <dd>{overview.data.comments}</dd>
                    </div>
                  </dl>
                  <p>
                    Artists, artwork and personal collections are managed in each server’s
                    Community.
                  </p>
                  <a className="button secondary" href="/community">
                    Open Community
                    <ArrowUpRight size={15} />
                  </a>
                </section>
              </div>
            </>
          )}
          {section === 'servers' && servers.data && (
            <div className="admin-server-list">
              {servers.data.length === 0 && (
                <Empty
                  title="No servers connected"
                  text="Connect the bot to a server through Community to get started."
                />
              )}
              {servers.data.map((s) => (
                <article className="admin-panel admin-server" key={s.id}>
                  <div className="admin-server-initial">{s.name.slice(0, 1)}</div>
                  <div>
                    <h2>{s.name}</h2>
                    <p>
                      {s.platform} · {s.artists} artists · {s.webhooks} webhooks
                    </p>
                    <small>
                      {s.channel_name ? '#' + s.channel_name : 'No bot channel'} · Connected{' '}
                      {date(s.created_at)}
                    </small>
                  </div>
                  <span className={`badge ${s.enabled ? 'good' : ''}`}>
                    {s.enabled ? 'Delivery enabled' : 'Delivery paused'}
                  </span>
                </article>
              ))}
            </div>
          )}
          {section === 'queues' && queues.data && (
            <div className="admin-queue-list">
              {queues.data.length === 0 && (
                <Empty
                  title="Nothing in the queue"
                  text="Bot deliveries, webhooks and media tasks will appear here."
                />
              )}
              {queues.data.map((j) => (
                <article className="admin-panel admin-job" key={j.kind + j.id}>
                  <div>
                    <span className="eyebrow">{j.kind}</span>
                    <h2>{j.server}</h2>
                    <p>
                      {date(j.created_at)} · {j.attempts} attempts
                    </p>
                    {j.last_error && <p className="error">{j.last_error}</p>}
                  </div>
                  <span
                    className={`badge ${j.status === 'failed' ? 'danger' : j.status === 'done' ? 'good' : ''}`}
                  >
                    {j.status}
                  </span>
                  {j.status === 'failed' && (
                    <button
                      className="button secondary"
                      disabled={busy}
                      onClick={() => void act(`/admin/queues/${j.kind}/${j.id}/retry`)}
                    >
                      Retry
                    </button>
                  )}
                </article>
              ))}
            </div>
          )}
          {section === 'services' && services.data && (
            <div className="admin-panels">
              {[
                {
                  name: 'PostgreSQL',
                  value: services.data.database,
                  detail: 'Application database connectivity.',
                },
                {
                  name: 'Discord',
                  value: services.data.discord,
                  detail: 'HTTP interactions and bot delivery configuration.',
                },
                {
                  name: 'RSSHub',
                  value: services.data.rsshub,
                  detail:
                    'Feed builder configured. Individual fetch failures appear in the overview.',
                },
                {
                  name: 'Feed scheduler',
                  value: date(services.data.last_poll),
                  detail: 'Most recent completed feed check.',
                },
              ].map((s) => (
                <section className="admin-panel" key={s.name}>
                  <h2>{s.name}</h2>
                  <strong className="admin-service-value">{s.value}</strong>
                  <p>{s.detail}</p>
                </section>
              ))}
            </div>
          )}
          {section === 'settings' && settings.data && (
            <GlobalSettings
              settings={settings.data}
              busy={busy}
              save={(v) => act('/admin/settings', v)}
            />
          )}
          <footer className="admin-footer">
            Installation administration · Community content stays in Community.
          </footer>
        </div>
      </main>
    </div>
  );
}
function Empty({ title, text }: { title: string; text: string }) {
  return (
    <div className="admin-empty">
      <CheckCircle2 size={32} />
      <h2>{title}</h2>
      <p>{text}</p>
    </div>
  );
}
function GlobalSettings({
  settings,
  busy,
  save,
}: {
  settings: Settings;
  busy: boolean;
  save: (v: Settings) => Promise<boolean>;
}) {
  const [value, setValue] = useState(settings);
  const [saved, setSaved] = useState(false);
  return (
    <form
      className="admin-panel admin-settings"
      onSubmit={async (e) => {
        e.preventDefault();
        setSaved(await save(value));
      }}
    >
      <h2>Global automation</h2>
      <p>These switches apply to all servers. Paused jobs stay queued.</p>
      <Toggle
        checked={value.notifications_enabled}
        onChange={(v) => setValue({ ...value, notifications_enabled: v })}
        label="Allow notifications"
        description="Master switch for bot and webhook deliveries."
      />
      <Toggle
        checked={value.downloads_enabled}
        onChange={(v) => setValue({ ...value, downloads_enabled: v })}
        label="Allow media downloads"
        description="Allow server administrators to download individual posts manually."
      />
      <button className="button primary" disabled={busy}>
        Save settings
      </button>
      {saved && <p role="status">Settings saved.</p>}
    </form>
  );
}
