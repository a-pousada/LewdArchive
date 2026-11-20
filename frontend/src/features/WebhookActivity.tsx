import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { communityApi as api } from './communityApi';
import { date } from '../api';
export default function WebhookActivity({ serverId }: { serverId: string }) {
  const base = `/servers/${serverId}/webhook-deliveries`;
  const client = useQueryClient();
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const data = useQuery({
    queryKey: ['community', serverId, 'webhook-deliveries'],
    queryFn: () =>
      api<
        {
          id: string;
          title: string;
          destination: string;
          status: string;
          attempts: number;
          last_error: string | null;
          created_at: string;
        }[]
      >(base),
    refetchInterval: 15000,
  });
  return (
    <section className="community-activity">
      <h2>Webhook deliveries</h2>
      {(error || data.isError) && (
        <p className="error" role="alert">
          {error || data.error?.message}
        </p>
      )}
      {data.isPending && <p role="status">Loading deliveries…</p>}
      {data.data?.length === 0 && (
        <p className="notice">
          No webhook deliveries yet. New posts from tracked artists will appear here.
        </p>
      )}
      {data.data?.map((d) => (
        <article key={d.id}>
          <div>
            <strong>{d.title}</strong>
            <p>
              {d.destination} · {date(d.created_at)}
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
              onClick={async () => {
                setBusy(true);
                setError('');
                try {
                  await api(`${base}/${d.id}/retry`, 'POST');
                  await client.invalidateQueries({
                    queryKey: ['community', serverId, 'webhook-deliveries'],
                  });
                } catch (e) {
                  setError((e as Error).message);
                } finally {
                  setBusy(false);
                }
              }}
            >
              Retry
            </button>
          )}
        </article>
      ))}
      <h2>Bot deliveries</h2>
    </section>
  );
}
