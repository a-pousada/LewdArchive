import { useState } from 'react';
import { Archive, ArrowUpRight, LoaderCircle } from 'lucide-react';
import { api } from '../api';
import { Logo } from '../components';
export default function Login({ onLogin }: { onLogin: () => void }) {
  const [token, setToken] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  return (
    <div className="login-page">
      <div className="login-brand">
        <Logo />
        <div className="login-message">
          <span className="eyebrow">INSTALLATION ADMINISTRATION</span>
          <h1>
            One installation.
            <br />
            Every <em>community.</em>
          </h1>
          <p>
            Monitor services and delivery.
            <br />
            Manage global automation.
            <br />
            Keep your communities running.
          </p>
          <div className="login-orbit">
            <Archive size={88} strokeWidth={0.7} />
            <span>ARCHIVE</span>
          </div>
        </div>
        <small>RESTRICTED TO THE INSTALLATION ADMINISTRATOR.</small>
      </div>
      <div className="login-form">
        <span className="eyebrow">BOT ADMINISTRATOR</span>
        <h2>Bot administration.</h2>
        <p>Restricted to the bot administrator. Sign in with your installation’s access key.</p>
        <form
          onSubmit={async (e) => {
            e.preventDefault();
            setBusy(true);
            setError('');
            try {
              await api('/login', 'POST', { token });
              onLogin();
            } catch (e) {
              setError((e as Error).message);
            } finally {
              setBusy(false);
            }
          }}
        >
          <label>
            Access key
            <input
              type="password"
              autoComplete="current-password"
              required
              value={token}
              onChange={(e) => setToken(e.target.value)}
              placeholder="Your access key"
            />
          </label>
          {error && (
            <p className="error" role="alert">
              {error}
            </p>
          )}
          <button className="button primary" disabled={busy}>
            {busy ? <LoaderCircle size={17} className="spin" /> : <ArrowUpRight size={17} />}Sign in
            to LewdArchive
          </button>
        </form>
        <a className="community-admin" href="/community">
          Looking for your server? Continue with Discord →
        </a>
        <p className="hint">
          Use the <code>API_TOKEN</code> value from your installation’s <code>.env</code> file.
        </p>
      </div>
    </div>
  );
}
