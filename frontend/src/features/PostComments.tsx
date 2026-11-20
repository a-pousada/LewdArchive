import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { MessageCircle, Send, Trash2, ChevronLeft, ChevronRight } from 'lucide-react';
import { Modal } from '../components';
import { date } from '../api';
import { communityApi as api } from './communityApi';
import type { Post } from './communityApi';
interface Comment {
  id: string;
  body: string;
  created_at: string;
  can_delete: boolean;
  author: { id: string; platform: string; display_name: string; avatar_url: string };
}
interface Thread {
  items: Comment[];
  total: number;
  page: number;
  pages: number;
}
export default function PostComments({ serverId, post }: { serverId: string; post: Post }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        className="comment-trigger"
        onClick={() => setOpen(true)}
        aria-label={`Comments on ${post.title}`}
      >
        <MessageCircle size={17} />
        Comments
      </button>
      {open && <ThreadPanel serverId={serverId} post={post} onClose={() => setOpen(false)} />}
    </>
  );
}
function ThreadPanel({
  serverId,
  post,
  onClose,
}: {
  serverId: string;
  post: Post;
  onClose: () => void;
}) {
  return (
    <Modal wide title="Conversation" subtitle={post.title} onClose={onClose}>
      <CommentThread serverId={serverId} post={post} />
    </Modal>
  );
}
export function CommentThread({ serverId, post }: { serverId: string; post: Post }) {
  const client = useQueryClient();
  const [page, setPage] = useState(1);
  const [body, setBody] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [removing, setRemoving] = useState<string | null>(null);
  const base = `/servers/${serverId}/posts/${post.id}/comments`;
  const key = ['community', serverId, 'comments', post.id];
  const thread = useQuery({
    queryKey: [...key, page],
    queryFn: () => api<Thread>(`${base}?page=${page}`),
  });
  async function mutate(path: string, method: string, value?: unknown) {
    setBusy(true);
    setError('');
    try {
      await api(path, method, value);
      if (method === 'POST') {
        setBody('');
        setPage(1);
      }
      setRemoving(null);
      await client.invalidateQueries({ queryKey: key });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="post-discussion">
      <div className="discussion-context">
        <MessageCircle size={24} />
        <div>
          <strong>{post.feed_name}</strong>
          <p>Visible only to members of this server.</p>
        </div>
        <a href={post.url} target="_blank" rel="noreferrer">
          Original post ↗
        </a>
      </div>
      <form
        className="comment-composer"
        onSubmit={(e) => {
          e.preventDefault();
          void mutate(base, 'POST', { body });
        }}
      >
        <label htmlFor="comment-body">Add to the conversation</label>
        <textarea
          id="comment-body"
          placeholder="What caught your eye?"
          value={body}
          maxLength={2000}
          rows={4}
          required
          onChange={(e) => setBody(e.target.value)}
        />
        <div>
          <small>
            {body.length.toLocaleString()} / 2,000 · Your signed-in name and photo appear with your
            comment.
          </small>
          <button className="button primary" disabled={busy || !body.trim()}>
            <Send size={15} />
            Post comment
          </button>
        </div>
      </form>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <div className="discussion-heading">
        <h3>{thread.data?.total ?? '…'} comments</h3>
        <span>Newest first</span>
      </div>
      {thread.isPending && <p role="status">Loading conversation…</p>}
      {thread.isError && (
        <p className="error" role="alert">
          {thread.error.message} <button onClick={() => void thread.refetch()}>Try again</button>
        </p>
      )}
      {thread.data?.items.length === 0 && (
        <div className="discussion-empty">
          <MessageCircle size={30} />
          <h3>Start the conversation.</h3>
          <p>Share a detail you loved, a thought, or a question about this post.</p>
        </div>
      )}
      <div className="comment-list">
        {thread.data?.items.map((c) => (
          <article className="comment" key={c.id}>
            <img
              src={c.author.avatar_url || '/favicon.svg'}
              alt=""
              referrerPolicy="no-referrer"
              onError={(e) => {
                e.currentTarget.onerror = null;
                e.currentTarget.src = '/favicon.svg';
              }}
            />
            <div className="comment-content">
              <header>
                <strong>{c.author.display_name}</strong>
                <span className="comment-provider">{c.author.platform}</span>
                <time dateTime={c.created_at}>{date(c.created_at)}</time>
              </header>
              <p>{c.body}</p>
              {c.can_delete &&
                (removing === c.id ? (
                  <div className="comment-removal">
                    <span>Delete this comment permanently?</span>
                    <button
                      disabled={busy}
                      onClick={() => void mutate(`${base}/${c.id}`, 'DELETE')}
                    >
                      Delete
                    </button>
                    <button disabled={busy} onClick={() => setRemoving(null)}>
                      Cancel
                    </button>
                  </div>
                ) : (
                  <button
                    className="comment-delete"
                    aria-label={`Delete comment by ${c.author.display_name}`}
                    onClick={() => setRemoving(c.id)}
                  >
                    <Trash2 size={13} />
                    Delete
                  </button>
                ))}
            </div>
          </article>
        ))}
      </div>
      {thread.data && thread.data.pages > 1 && (
        <div className="community-pagination">
          <button
            className="button secondary"
            disabled={page === 1}
            onClick={() => setPage((p) => p - 1)}
          >
            <ChevronLeft size={16} />
            Previous
          </button>
          <span>
            {page} / {thread.data.pages}
          </span>
          <button
            className="button secondary"
            disabled={page >= thread.data.pages}
            onClick={() => setPage((p) => p + 1)}
          >
            Next
            <ChevronRight size={16} />
          </button>
        </div>
      )}
      <p className="hint">
        You can delete your own comments. Server administrators can moderate this conversation.
      </p>
    </section>
  );
}
