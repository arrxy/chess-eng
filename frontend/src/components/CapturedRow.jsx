import { GLYPH } from '../chess.js';

// `pieces` were captured by one side, so they belong to `victimColor`.
export default function CapturedRow({ pieces, victimColor, adv }) {
  return (
    <span className="cap-row">
      {(pieces || []).map((t, i) => (
        <span key={i} className={`cap-pc ${victimColor === 'white' ? 'w' : 'b'}`}>
          {GLYPH[victimColor][t]}
        </span>
      ))}
      {adv > 0 && <span className="cap-adv">+{adv}</span>}
    </span>
  );
}
