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
          <span className="eyebrow">YOUR PERSONAL COLLECTION / KEEP WHAT YOU LOVE</span>
          <h1>
            One archive.
            <br />
            Endless <em>discoveries.</em>
          </h1>
          <p>
            The artists you follow.
            <br />
            The artwork you want to keep.
            <br />
            All in your own space.
          </p>
          <div className="login-orbit">
            <Archive size={88} strokeWidth={0.7} />
            <span>ARCHIVE</span>
          </div>
        </div>
        <small>MADE FOR COLLECTORS OF INSPIRATION.</small>
      </div>
      <div className="login-form">
        <span className="eyebrow">YOUR PERSONAL OBSERVATORY</span>
        <h2>Welcome to your archive.</h2>
        <p>Sign in with your installation’s access key.</p>
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
        <p className="hint">
          Use the <code>API_TOKEN</code> value from your installation’s <code>.env</code> file.
        </p>
      </div>
    </div>
  );
}
