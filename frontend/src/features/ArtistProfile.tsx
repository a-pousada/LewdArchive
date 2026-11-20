import { useState } from 'react';
import { CategoryField } from './Categories';
import ArtistProfileFields from './ArtistProfileFields';
import { Modal } from '../components';
import type { Artist } from './communityApi';
export default function ArtistProfile({
  artist,
  onClose,
  save,
  categories,
}: {
  artist: Artist;
  categories?: string[];
  onClose: () => void;
  save: (value: unknown) => Promise<boolean>;
}) {
  const [name, setName] = useState(artist.name);
  const [category, setCategory] = useState(artist.category || '');
  const [avatar, setAvatar] = useState(artist.avatar_url || '');
  const [links, setLinks] = useState(artist.official_links || []);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  return (
    <Modal
      title="Artist profile"
      subtitle="Category, photo and official pages for this server’s library."
      onClose={onClose}
    >
      <form
        onSubmit={async (e) => {
          e.preventDefault();
          setBusy(true);
          setError('');
          try {
            if (await save({ name, category, avatar_url: avatar, official_links: links }))
              onClose();
            else setError('Could not save the profile. Check the URLs and try again.');
          } finally {
            setBusy(false);
          }
        }}
      >
        <label>
          Artist name
          <input required maxLength={120} value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <CategoryField value={category} onChange={setCategory} categories={categories} />
        <ArtistProfileFields
          name={name}
          avatar={avatar}
          links={links}
          onAvatar={setAvatar}
          onLinks={setLinks}
        />
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
        <div className="modal-actions">
          <button type="button" className="button secondary" onClick={onClose}>
            Cancel
          </button>
          <button className="button primary" disabled={busy}>
            {busy ? 'Saving…' : 'Save profile'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
