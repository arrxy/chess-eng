// layouts.jsx — the four wall layouts + shared pieces. Exports to window.
const { MiniBoard, EvalBar, fmtEval, lastMoveGlyph } = window;

// one player line: live dot when it's their move, name, clock
function Player({ name, clock, running, showClocks, big }) {
  return (
    <div className={'prow' + (running ? ' on' : '') + (big ? ' big' : '')}>
      <span className="turn-dot" aria-hidden="true"></span>
      <span className="pname">{name}</span>
      {showClocks && <span className={'clock' + (running ? ' tick' : '')}>{clock}</span>}
    </div>
  );
}

function LastMove({ game, showLast }) {
  if (!showLast) return null;
  const moverWhite = game.turn === 'b';
  const glyph = lastMoveGlyph(game.last, moverWhite);
  return (
    <div className="lastmove">
      <span className="lm-label">last</span>
      <span className="lm-move">{glyph && <span className="lm-glyph">{glyph}</span>}{game.last}</span>
      <span className={'lm-ev ' + (game.ev > 0.15 ? 'pos' : game.ev < -0.15 ? 'neg' : '')}>{fmtEval(game.ev)}</span>
    </div>
  );
}

function LiveTag() {
  return <span className="live"><span className="live-dot"></span>live</span>;
}

// ---- A. uniform grid wall ----
function GameCard({ game, opts }) {
  return (
    <div className="card game-card">
      <div className="board-wrap">
        <MiniBoard fen={game.fen} />
        <LiveTag />
      </div>
      <div className="meta">
        <Player name={game.b} clock={game.bt} running={game.turn === 'b'} showClocks={opts.showClocks} />
        <Player name={game.w} clock={game.wt} running={game.turn === 'w'} showClocks={opts.showClocks} />
        <LastMove game={game} showLast={opts.showLast} />
      </div>
    </div>
  );
}

function GridWall({ games, opts }) {
  const tile = { cozy: '262px', regular: '212px', dense: '168px' }[opts.density];
  return (
    <div className="grid-wall" style={{ '--tile': tile }}>
      {games.map((g) => <GameCard key={g.id} game={g} opts={opts} />)}
    </div>
  );
}

// ---- B. spotlight + rail ----
function Spotlight({ games, opts }) {
  const main = games[0];
  const rest = games.slice(1);
  const moverWhite = main.turn === 'b';
  return (
    <div className="spot">
      <div className="card spot-main">
        <div className="spot-head">
          <LiveTag />
          <span className="spot-open">Italian Game</span>
        </div>
        <div className="spot-body">
          <EvalBar ev={main.ev} />
          <div className="spot-board"><MiniBoard fen={main.fen} coords /></div>
          <div className="spot-side">
            <Player name={main.b} clock={main.bt} running={main.turn === 'b'} showClocks={opts.showClocks} big />
            <div className="spot-lastbox">
              <span className="lm-label">last move</span>
              <span className="spot-last">{lastMoveGlyph(main.last, moverWhite)} {main.last}</span>
              <span className="spot-evtxt">{fmtEval(main.ev)}</span>
            </div>
            <Player name={main.w} clock={main.wt} running={main.turn === 'w'} showClocks={opts.showClocks} big />
          </div>
        </div>
      </div>
      <div className="spot-rail">
        {rest.map((g) => (
          <div key={g.id} className="card rail-card">
            <MiniBoard fen={g.fen} />
            <div className="rail-meta">
              <div className="rail-names">
                <span className={g.turn === 'b' ? 'on' : ''}>{g.b}</span>
                <span className={g.turn === 'w' ? 'on' : ''}>{g.w}</span>
              </div>
              {opts.showClocks && (
                <div className="rail-clocks">
                  <span className={g.turn === 'b' ? 'on' : ''}>{g.bt}</span>
                  <span className={g.turn === 'w' ? 'on' : ''}>{g.wt}</span>
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ---- C. compact scannable strips ----
function Strips({ games, opts }) {
  return (
    <div className="strips">
      <div className="strip strip-head">
        <span>board</span>
        <span>players</span>
        {opts.showClocks && <span>clock</span>}
        {opts.showLast && <span>last</span>}
        <span>eval</span>
      </div>
      {games.map((g) => {
        const moverWhite = g.turn === 'b';
        return (
          <div key={g.id} className="strip card">
            <div className="strip-board"><MiniBoard fen={g.fen} /></div>
            <div className="strip-players">
              <span className={g.turn === 'b' ? 'pl on' : 'pl'}><i className="d"></i>{g.b}</span>
              <span className="vs">vs</span>
              <span className={g.turn === 'w' ? 'pl on' : 'pl'}><i className="d"></i>{g.w}</span>
            </div>
            {opts.showClocks && (
              <div className="strip-clocks">
                <span className={g.turn === 'b' ? 'on' : ''}>{g.bt}</span>
                <span className={g.turn === 'w' ? 'on' : ''}>{g.wt}</span>
              </div>
            )}
            {opts.showLast && (
              <div className="strip-last">{lastMoveGlyph(g.last, moverWhite)} {g.last}</div>
            )}
            <div className="strip-eval">
              <EvalBar ev={g.ev} />
              <span className="strip-evtxt">{fmtEval(g.ev)}</span>
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ---- D. bare board wall ----
function BareWall({ games, opts }) {
  const tile = { cozy: '210px', regular: '162px', dense: '124px' }[opts.density];
  return (
    <div className="bare-wall" style={{ '--tile': tile }}>
      {games.map((g) => (
        <div key={g.id} className="bare-tile">
          <MiniBoard fen={g.fen} />
          <div className="bare-cap">
            <span className={g.turn === 'b' ? 'on' : ''}>{g.b}</span>
            <span className="bare-vs">/</span>
            <span className={g.turn === 'w' ? 'on' : ''}>{g.w}</span>
            {opts.showClocks && <span className="bare-clk">{g.turn === 'w' ? g.wt : g.bt}</span>}
          </div>
        </div>
      ))}
    </div>
  );
}

Object.assign(window, { GridWall, Spotlight, Strips, BareWall });
