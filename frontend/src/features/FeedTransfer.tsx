import { useState } from 'react';
import { FileUp, Download, LoaderCircle, CheckCircle2 } from 'lucide-react';
import { communityApi as api } from './communityApi';
import { Modal, Toggle } from '../components';

type Preview = {
  new: number;
  duplicates: number;
  invalid: number;
  feeds: {
    name: string;
    url: string;
    category: string;
    enabled: boolean;
    status: string;
    error: string | null;
  }[];
};
type ImportResult = { imported: number; duplicates: number; invalid: number };
export default function FeedTransfer({
  serverId,
  onImported,
}: {
  serverId: string;
  onImported: () => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <div className="feed-transfer-bar">
        <div>
          <strong>Take your subscriptions with you.</strong>
          <p>Import from Miniflux or another reader. Export your collection as OPML.</p>
        </div>
        <div className="transfer-actions">
          <a
            className="button secondary"
            href={`/api/community/servers/${serverId}/feeds/export`}
            download="lewdarchive-feeds.opml"
          >
            <Download size={16} />
            Export OPML
          </a>
          <button className="button secondary" onClick={() => setOpen(true)}>
            <FileUp size={16} />
            Import OPML
          </button>
        </div>
      </div>
      {open && (
        <ImportDialog serverId={serverId} onClose={() => setOpen(false)} onImported={onImported} />
      )}
    </>
  );
}
function ImportDialog({
  serverId,
  onClose,
  onImported,
}: {
  serverId: string;
  onClose: () => void;
  onImported: () => void;
}) {
  const [opml, setOpml] = useState('');
  const [filename, setFilename] = useState('');
  const [preview, setPreview] = useState<Preview | null>(null);
  const [result, setResult] = useState<ImportResult | null>(null);
  const [enabled, setEnabled] = useState(true);
  const [minutes, setMinutes] = useState(15);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  return (
    <Modal
      wide
      title="Import your feeds"
      subtitle="An OPML file from Miniflux or another feed reader."
      onClose={() => !busy && onClose()}
    >
      {result ? (
        <div className="import-complete" role="status">
          <CheckCircle2 size={36} />
          <h3>{result.imported} feeds imported</h3>
          <p>
            {result.duplicates} duplicates skipped · {result.invalid} invalid feeds skipped
          </p>
          <p>The first sync imports history silently. Existing feeds were kept as they were.</p>
          <button className="button primary" onClick={onClose}>
            Done
          </button>
        </div>
      ) : (
        <>
          <label className="file-picker">
            <FileUp size={28} />
            <strong>{filename || 'Choose your OPML file'}</strong>
            <span>OPML or XML · up to 2 MiB · 1,000 feeds</span>
            <input
              aria-label="OPML file"
              type="file"
              accept=".opml,.xml,text/xml,application/xml,text/x-opml"
              disabled={busy}
              onChange={async (e) => {
                const file = e.target.files?.[0];
                setPreview(null);
                setOpml('');
                setError('');
                setFilename(file?.name || '');
                if (!file) return;
                if (file.size > 2 * 1024 * 1024) {
                  setError('OPML files must be no larger than 2 MiB.');
                  return;
                }
                setBusy(true);
                try {
                  const text = await file.text();
                  const data = await api<Preview>(
                    `/servers/${serverId}/feeds/import/preview`,
                    'POST',
                    { opml: text },
                  );
                  setOpml(text);
                  setPreview(data);
                } catch (e) {
                  setError((e as Error).message);
                } finally {
                  setBusy(false);
                }
              }}
            />
          </label>
          {busy && !preview && (
            <p className="hint" role="status">
              Reading your subscriptions...
            </p>
          )}
          {preview && (
            <>
              <div className="import-summary" aria-live="polite">
                <span>
                  <strong>{preview.new}</strong> new
                </span>
                <span>
                  <strong>{preview.duplicates}</strong> duplicates
                </span>
                <span>
                  <strong>{preview.invalid}</strong> invalid
                </span>
              </div>
              <div className="import-feed-list">
                {preview.feeds.map((feed, index) => (
                  <div className="import-feed-row" key={index}>
                    <div>
                      <strong>{feed.name}</strong>
                      <small>
                        {feed.category || 'Uncategorized'}
                        {!feed.enabled && ' · Paused in the original file'}
                      </small>
                      <code>{feed.url}</code>
                      {feed.error && <small className="error">{feed.error}</small>}
                    </div>
                    <span
                      className={`badge ${feed.status === 'new' ? 'good' : feed.status === 'invalid' ? 'danger' : ''}`}
                    >
                      {feed.status}
                    </span>
                  </div>
                ))}
                {!preview.feeds.length && <p>No feed subscriptions found in this file.</p>}
              </div>
              <p className="hint">
                Categories and feed URLs are preserved. Nested categories become a single path.
                Miniflux rules, credentials, and article history are not imported.
              </p>
              <div className="import-options">
                <Toggle
                  checked={enabled}
                  onChange={setEnabled}
                  label="Start following imported feeds"
                  description="Feeds paused in the OPML file stay paused."
                />
                <label>
                  Check every
                  <select value={minutes} onChange={(e) => setMinutes(Number(e.target.value))}>
                    {[5, 15, 30, 60, 360, 1440].map((m) => (
                      <option key={m} value={m}>
                        {m < 60 ? `${m} minutes` : `${m / 60} hours`}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
            </>
          )}
          {error && (
            <p className="error" role="alert">
              {error}
            </p>
          )}
          <div className="modal-actions">
            <button className="button secondary" disabled={busy} onClick={onClose}>
              Cancel
            </button>
            <button
              className="button primary"
              disabled={busy || !preview?.new}
              onClick={async () => {
                setBusy(true);
                setError('');
                try {
                  const imported = await api<ImportResult>(
                    `/servers/${serverId}/feeds/import`,
                    'POST',
                    {
                      opml,
                      enabled,

                      poll_minutes: minutes,
                    },
                  );
                  setResult(imported);
                  onImported();
                } catch (e) {
                  setError((e as Error).message);
                } finally {
                  setBusy(false);
                }
              }}
            >
              {busy ? <LoaderCircle className="spin" size={16} /> : <FileUp size={16} />}Import{' '}
              {preview?.new || 0} feeds
            </button>
          </div>
        </>
      )}
    </Modal>
  );
}
