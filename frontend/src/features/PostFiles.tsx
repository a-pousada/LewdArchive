import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Download } from 'lucide-react';
import { Modal } from '../components';
import { bytes } from '../api';
import { communityApi as api } from './communityApi';
import type { Post } from './communityApi';
export default function PostFiles({
  serverId,
  post,
  manager,
}: {
  serverId: string;
  post: Post;
  manager: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const client = useQueryClient();
  const base = `/servers/${serverId}/posts/${post.id}`;
  const files = useQuery({
    queryKey: ['community', serverId, 'files', post.id],
    queryFn: () => api<{ id: string; filename: string; size_bytes: number }[]>(`${base}/downloads`),
    enabled: open,
    refetchInterval: open ? 5000 : false,
  });
  return (
    <>
      <button
        className="comment-trigger"
        onClick={() => setOpen(true)}
        aria-label={`Files for ${post.title}`}
      >
        <Download size={17} />
        Files
      </button>
      {open && (
        <Modal title="Post files" subtitle={post.title} onClose={() => setOpen(false)}>
          {files.isPending && <p role="status">Loading files…</p>}
          {files.isError && (
            <p className="error" role="alert">
              {files.error.message}
            </p>
          )}
          {files.data?.length === 0 && <p className="notice">No stored files for this post yet.</p>}
          {files.data?.map((f) => (
            <p key={f.id}>
              <a
                className="button secondary"
                href={`/api/community/servers/${serverId}/downloads/${f.id}/file`}
              >
                <Download size={16} />
                {f.filename} · {bytes(f.size_bytes)}
              </a>
            </p>
          ))}
          {manager && post.media.length > 0 && (
            <button
              className="button primary"
              disabled={busy}
              onClick={async () => {
                setBusy(true);
                setError('');
                try {
                  await api(`${base}/downloads`, 'POST');
                  setNotice('Download queued. Files appear here when ready.');
                  await client.invalidateQueries({
                    queryKey: ['community', serverId, 'files', post.id],
                  });
                } catch (e) {
                  setError((e as Error).message);
                } finally {
                  setBusy(false);
                }
              }}
            >
              Download post media
            </button>
          )}
          {manager && (
            <button
              className="button secondary"
              disabled={busy}
              onClick={async () => {
                setBusy(true);
                setError('');
                try {
                  const result = await api<{ queued: number }>(`${base}/webhooks`, 'POST');
                  setNotice(
                    result.queued ? 'Webhook delivery queued.' : 'This post is already queued.',
                  );
                } catch (e) {
                  setError((e as Error).message);
                } finally {
                  setBusy(false);
                }
              }}
            >
              Send to server webhooks
            </button>
          )}
          {notice && <p role="status">{notice}</p>}
          {error && (
            <p className="error" role="alert">
              {error}
            </p>
          )}
        </Modal>
      )}
    </>
  );
}
