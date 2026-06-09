// app.jsx — tab shell + tweaks. Mounts the wall.
const { GAMES, GridWall, Spotlight, Strips, BareWall } = window;

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "accent": "#3b6ea5",
  "theme": "warm",
  "density": "regular",
  "sketch": true,
  "showClocks": true,
  "showLast": true
}/*EDITMODE-END*/;

const TABS = [
  { id: 'grid', label: 'Grid wall', note: 'even tiles, everything equal' },
  { id: 'spot', label: 'Spotlight', note: 'one feature + a rail' },
  { id: 'strips', label: 'Strips', note: 'dense, scannable rows' },
  { id: 'bare', label: 'Bare wall', note: 'just boards' }
];

function App() {
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);
  const [tab, setTab] = React.useState('grid');

  React.useEffect(() => {
    const root = document.documentElement;
    root.style.setProperty('--accent', t.accent);
    root.setAttribute('data-theme', t.theme);
    root.setAttribute('data-sketch', t.sketch ? 'on' : 'off');
  }, [t.accent, t.theme, t.sketch]);

  const opts = { density: t.density, showClocks: t.showClocks, showLast: t.showLast };
  const Wall = { grid: GridWall, spot: Spotlight, strips: Strips, bare: BareWall }[tab];

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="brand-name">parallel</span>
          <span className="brand-sub">&mdash; a wall of games in motion</span>
        </div>
        <div className="topbar-right">
          <span className="livecount"><span className="live-dot"></span>{GAMES.length} live now</span>
        </div>
      </header>

      <nav className="tabs">
        {TABS.map((x) => (
          <button key={x.id} className={'tab' + (tab === x.id ? ' active' : '')} onClick={() => setTab(x.id)}>
            <span className="tab-label">{x.label}</span>
            <span className="tab-note">{x.note}</span>
          </button>
        ))}
        <span className="tabs-hint">wireframe &middot; 4 directions</span>
      </nav>

      <main className="wall">
        <Wall games={GAMES} opts={opts} />
      </main>

      <TweaksPanel>
        <TweakSection label="Look" />
        <TweakColor label="Accent" value={t.accent}
          options={['#3b6ea5', '#5a8f5a', '#c08a2e', '#2c2a27']}
          onChange={(v) => setTweak('accent', v)} />
        <TweakRadio label="Board tone" value={t.theme}
          options={['warm', 'grey']}
          onChange={(v) => setTweak('theme', v)} />
        <TweakToggle label="Hand-drawn frames" value={t.sketch}
          onChange={(v) => setTweak('sketch', v)} />
        <TweakSection label="Density" />
        <TweakRadio label="Tile size" value={t.density}
          options={['cozy', 'regular', 'dense']}
          onChange={(v) => setTweak('density', v)} />
        <TweakSection label="Per-board info" />
        <TweakToggle label="Clocks" value={t.showClocks}
          onChange={(v) => setTweak('showClocks', v)} />
        <TweakToggle label="Last move / eval" value={t.showLast}
          onChange={(v) => setTweak('showLast', v)} />
      </TweaksPanel>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
