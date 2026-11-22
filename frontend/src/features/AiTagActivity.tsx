import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { LoaderCircle, RefreshCw, Sparkles, Square } from 'lucide-react';
import { communityApi as api } from './communityApi';
import { date } from '../api';

interface AiJobsResponse {
  tagger_configured: boolean;
  ai_tags_enabled: boolean;
  counts: {
    pending: number;
    processing: number;
    done: number;
    failed: number;
    untagged_with_images: number;
  };
  recent: {
    entry_id: string;
    title: string;
    artist: string;
    status: string;
    attempts: number;
    last_error: string | null;
    created_at: string;
  }[];
}

export default function AiTagActivity({ serverId }: { serverId: string }) {
  const client = useQueryClient();
  const [batchSize, setBatchSize] = useState(200);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState('');
  const [error, setError] = useState('');

  const query = useQuery({
    queryKey: ['community', serverId, 'ai-jobs'],
    queryFn: () => api<AiJobsResponse>(`/servers/${serverId}/ai/jobs`),
    refetchInterval: (q) => {
      const counts = q.state.data?.counts;
      return counts && (counts.pending > 0 || counts.processing > 0) ? 5000 : 15000;
    },
  });

  async function queueBackfill() {
    setBusy(true);
    setError('');
    setStatus('');
    try {
      const res = await api<{ queued: number }>(`/servers/${serverId}/ai/backfill`, 'POST', {
        limit: batchSize,
      });
      setStatus(`Queued ${res.queued} historical post(s) for WD image tagging.`);
      await client.invalidateQueries({ queryKey: ['community', serverId, 'ai-jobs'] });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  async function retryJobs(entryId?: string) {
    setBusy(true);
    setError('');
    setStatus('');
    try {
      const res = await api<{ retried: number }>(
        `/servers/${serverId}/ai/jobs/retry`,
        'POST',
        entryId ? { entry_id: entryId } : {},
      );
      setStatus(`Queued ${res.retried} failed AI job(s) for retry.`);
      await client.invalidateQueries({ queryKey: ['community', serverId, 'ai-jobs'] });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  async function cancelPending() {
    setBusy(true);
    setError('');
    setStatus('');
    try {
      const res = await api<{ cancelled: number }>(
        `/servers/${serverId}/ai/jobs/cancel-pending`,
        'POST',
      );
      setStatus(`Cancelled ${res.cancelled} pending backfill job(s).`);
      await client.invalidateQueries({ queryKey: ['community', serverId, 'ai-jobs'] });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }

  const data = query.data;
  const counts = data?.counts;
  const canBackfill = !!data?.tagger_configured && !!data?.ai_tags_enabled;

  return (
    <section className="settings-card obs-panel" aria-label="AI tagging queue">
      <div className="obs-header">
        <div>
          <h2>
            <Sparkles size={18} /> AI tagging & backfill
          </h2>
          <p className="hint">
            Queue historical posts with images in controlled batches without waking idle VRAM until
            needed.
          </p>
        </div>
      </div>

      {query.isPending && <p role="status">Loading AI queue…</p>}
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

      {data && !data.tagger_configured && (
        <p className="notice">
          WD Tagger is not configured on the server (`WD_TAGGER_URL`). Historical backfill is
          unavailable.
        </p>
      )}
      {data && data.tagger_configured && !data.ai_tags_enabled && (
        <p className="notice">
          Enable <strong>Automatically tag new posts</strong> in Server settings to run AI tagging
          and historical backfill batches.
        </p>
      )}

      {counts && (
        <>
          <div className="obs-stats">
            <div className="obs-stat">
              <strong>{counts.untagged_with_images}</strong>
              <span>Untagged w/ images</span>
            </div>
            <div className="obs-stat">
              <strong>{counts.pending}</strong>
              <span>Pending</span>
            </div>
            <div className="obs-stat">
              <strong>{counts.processing}</strong>
              <span>Processing</span>
            </div>
            <div className="obs-stat">
              <strong>{counts.done}</strong>
              <span>Done</span>
            </div>
            <div className="obs-stat">
              <strong>{counts.failed}</strong>
              <span>Failed</span>
            </div>
          </div>

          <div className="obs-controls">
            <label>
              Batch size
              <select
                aria-label="Backfill batch size"
                value={batchSize}
                onChange={(e) => setBatchSize(Number(e.target.value))}
              >
                <option value={50}>50 posts</option>
                <option value={200}>200 posts</option>
                <option value={500}>500 posts</option>
                <option value={1000}>1000 posts</option>
              </select>
            </label>
            <button
              type="button"
              className="button primary"
              disabled={busy || !canBackfill || counts.untagged_with_images === 0}
              onClick={() => void queueBackfill()}
            >
              {busy ? <LoaderCircle className="spin" size={16} /> : <Sparkles size={16} />}
              Queue backfill batch
            </button>
            {counts.pending > 0 && (
              <button
                type="button"
                className="button secondary"
                disabled={busy}
                onClick={() => void cancelPending()}
              >
                <Square size={14} /> Stop pending
              </button>
            )}
            {counts.failed > 0 && (
              <button
                type="button"
                className="button secondary"
                disabled={busy}
                onClick={() => void retryJobs()}
              >
                <RefreshCw size={15} /> Retry failed ({counts.failed})
              </button>
            )}
          </div>
        </>
      )}

      {data && data.recent.length > 0 && (
        <div className="obs-list">
          {data.recent.slice(0, 15).map((job) => (
            <article key={job.entry_id} className="obs-row">
              <div>
                <strong>{job.title}</strong>
                <small>
                  {job.artist} · {date(job.created_at)}
                </small>
                {job.last_error && <p className="error">{job.last_error}</p>}
              </div>
              <span>
                {job.status} · {job.attempts} attempts
              </span>
              {job.status === 'failed' && (
                <button
                  type="button"
                  className="button secondary"
                  disabled={busy}
                  onClick={() => void retryJobs(job.entry_id)}
                >
                  Retry
                </button>
              )}
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
