import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Activity, RefreshCw } from 'lucide-react';
import { communityApi as api } from './communityApi';
import { date } from '../api';

interface FeedHealthResponse {
  summary: {
    total: number;
    enabled: number;
    healthy: number;
    failing: number;
    paused: number;
  };
  failing: {
    artist_id: string;
    name: string;
    source: string;
    category: string;
    consecutive_errors: number;
    last_error: string | null;
    last_checked_at: string | null;
    next_check_at: string | null;
  }[];
}

export default function FeedHealth({ serverId }: { serverId: string }) {
  const client = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');
  const query = useQuery({
    queryKey: ['community', serverId, 'feed-health'],
    queryFn: () => api<FeedHealthResponse>(`/servers/${serverId}/feeds/health`),
    refetchInterval: 30000,
  });

  async function retryAll() {
    setBusy(true);
    setError('');
    setStatus('');
    try {
      const res = await api<{ queued: number }>(`/servers/${serverId}/feeds/retry-failed`, 'POST');
      setStatus(`Queued ${res.queued} failing feed(s) for immediate sync.`);
      await client.invalidateQueries({ queryKey: ['community', serverId] });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  async function retryOne(artistId: string, name: string) {
    setBusy(true);
    setError('');
    setStatus('');
    try {
      await api(`/servers/${serverId}/artists/${artistId}/sync`, 'POST');
      setStatus(`Scheduled immediate sync for ${name}.`);
      await client.invalidateQueries({ queryKey: ['community', serverId, 'feed-health'] });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  const s = query.data?.summary;
  const failing = query.data?.failing ?? [];

  return (
    <section className="settings-card obs-panel" aria-label="Feed health">
      <div className="obs-header">
        <div>
          <h2>
            <Activity size={18} /> Feed health
          </h2>
          <p className="hint">Failing feeds apply automatic backoff up to 6 hours.</p>
        </div>
        {failing.length > 0 && (
          <button
            type="button"
            className="button secondary"
            disabled={busy}
            onClick={() => void retryAll()}
          >
            <RefreshCw size={15} /> Retry all failed
          </button>
        )}
      </div>

      {query.isPending && <p role="status">Loading feed status…</p>}
      {(error || query.isError) && (
        <p className="error" role="alert">
          {error || query.error?.message}
        </p>
      )}
      {status && (
        <p className="hint" role="status">
          {status}
        </p>
      )}

      {s && (
        <div className="obs-stats">
          <div className="obs-stat">
            <strong>{s.healthy}</strong>
            <span>Healthy</span>
          </div>
          <div className="obs-stat">
            <strong>{s.failing}</strong>
            <span>Failing</span>
          </div>
          <div className="obs-stat">
            <strong>{s.paused}</strong>
            <span>Paused feeds</span>
          </div>
          <div className="obs-stat">
            <strong>{s.total}</strong>
            <span>Total</span>
          </div>
        </div>
      )}

      {failing.length > 0 && (
        <div className="obs-list">
          {failing.map((f) => (
            <article key={f.artist_id} className="obs-row">
              <div>
                <strong>{f.name}</strong>
                <small>
                  {f.source}
                  {f.category ? ` · ${f.category}` : ''} · {f.consecutive_errors} error(s) · Next
                  check {date(f.next_check_at)}
                </small>
                {f.last_error && <p className="error">{f.last_error}</p>}
              </div>
              <button
                type="button"
                className="button secondary"
                disabled={busy}
                onClick={() => void retryOne(f.artist_id, f.name)}
              >
                Retry now
              </button>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
