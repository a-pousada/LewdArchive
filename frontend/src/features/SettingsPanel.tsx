import { useState } from 'react';
import type { FormEvent } from 'react';
import {
  ArrowDownToLine,
  Check,
  CheckCircle2,
  FolderHeart,
  LoaderCircle,
  Volume2,
} from 'lucide-react';
import type { Settings } from '../api';
import { Toggle } from '../components';
export default function SettingsPanel({
  initial,
  onSave,
}: {
  initial: Settings;
  onSave: (data: unknown) => Promise<boolean>;
}) {
  const [webhook, setWebhook] = useState('');
  const [remove, setRemove] = useState(false);
  const [download, setDownload] = useState(initial.downloads_enabled);
  const [notify, setNotify] = useState(initial.notifications_enabled);
  const [busy, setBusy] = useState(false);
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    const ok = await onSave({
      downloads_enabled: download,
      notifications_enabled: notify,
      ...(remove ? { webhook_url: '' } : webhook ? { webhook_url: webhook } : {}),
    });
    if (ok) {
      setWebhook('');
      setRemove(false);
    }
    setBusy(false);
  };
  return (
    <form className="settings-layout" onSubmit={submit}>
      <div>
        <section className="settings-card">
          <div className="settings-section-head">
            <span className="stat-icon discord-icon">
              <Volume2 size={21} />
            </span>
            <div>
              <h2>Connect to Discord</h2>
              <p>New posts arrive directly in your channel.</p>
            </div>
            <span className={`badge ${initial.webhook_configured ? 'good' : ''}`}>
              {initial.webhook_configured ? 'Configured' : 'Not configured'}
            </span>
          </div>
          <label>
            Webhook URL
            <input
              type="password"
              autoComplete="new-password"
              value={webhook}
              onChange={(e) => {
                setWebhook(e.target.value);
                setRemove(false);
              }}
              placeholder={
                initial.webhook_configured
                  ? 'Webhook saved. Enter a URL only to replace it.'
                  : 'https://discord.com/api/webhooks/...'
              }
            />
            <small>
              In Discord: Channel Settings → Integrations → Webhooks. The saved URL is never
              displayed again.
            </small>
          </label>
          {initial.webhook_configured && (
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={remove}
                onChange={(e) => setRemove(e.target.checked)}
              />
              Remove the saved webhook
            </label>
          )}
          <Toggle
            checked={notify}
            onChange={setNotify}
            label="Send notifications"
            description="When paused, jobs remain queued until you enable notifications again."
          />
        </section>
        <section className="settings-card">
          <div className="settings-section-head">
            <span className="stat-icon">
              <ArrowDownToLine size={21} />
            </span>
            <div>
              <h2>Preserve your discoveries</h2>
              <p>Choose when files are saved.</p>
            </div>
          </div>
          <Toggle
            checked={download}
            onChange={setDownload}
            label="Allow downloads"
            description="Global control for automatic and manual downloads. Turning this off pauses the queue."
          />
          <p className="hint">
            Enable automatic downloads in each feed’s settings. The initial history is imported
            without downloading files.
          </p>
        </section>
        <div className="settings-save">
          <span>Changes apply to upcoming processing cycles.</span>
          <button className="button primary" disabled={busy}>
            {busy ? <LoaderCircle className="spin" size={16} /> : <Check size={16} />}Save settings
          </button>
        </div>
      </div>
      <aside className="settings-aside">
        <span className="eyebrow">A LOOK AT YOUR CHANNEL</span>
        <h3>
          Small notifications.
          <br />
          Great discoveries.
        </h3>
        <div className="discord-preview">
          <img src="/favicon.svg" alt="" />
          <div>
            <strong>
              LewdArchive <span>APP</span>
            </strong>
            <div className="discord-embed">
              <small>ARTIST NAME</small>
              <h4>New post title</h4>
              <p>Description, images, and a direct link to the original post.</p>
              <div className="preview-image">
                <FolderHeart size={38} strokeWidth={1} />
                <span>Post artwork appears here</span>
              </div>
              <small>LewdArchive · Your feed</small>
            </div>
          </div>
        </div>
        <p>
          Illustrative preview. Up to 4 images per message, video links, and no automatic mentions.
        </p>
        <div className="settings-tip">
          <CheckCircle2 size={17} />
          <span>Failed deliveries are retried. Follow their progress in Activity.</span>
        </div>
      </aside>
    </form>
  );
}
