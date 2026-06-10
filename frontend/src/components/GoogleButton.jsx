import { useEffect, useRef } from 'react';

export default function GoogleButton({ clientId, onUser, onError }) {
  const ref = useRef(null);
  useEffect(() => {
    if (!clientId) return;
    let cancelled = false;
    const tryInit = () => {
      if (cancelled) return;
      if (!(window.google && window.google.accounts && window.google.accounts.id)) {
        setTimeout(tryInit, 200); // GIS script still loading
        return;
      }
      window.google.accounts.id.initialize({
        client_id: clientId,
        callback: async (resp) => {
          try {
            const r = await fetch('/auth/google', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({ credential: resp.credential }),
            });
            const data = await r.json();
            if (r.ok) onUser(data.user);
            else onError(data.error || 'Sign-in failed.');
          } catch {
            onError('Sign-in failed.');
          }
        },
      });
      window.google.accounts.id.renderButton(ref.current, {
        theme: 'outline', size: 'large', shape: 'pill', text: 'signin_with',
      });
    };
    tryInit();
    return () => { cancelled = true; };
  }, [clientId]);
  return <div className="gsi-wrap"><div ref={ref} /></div>;
}
