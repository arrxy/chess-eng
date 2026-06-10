import { useState, useRef } from 'react';

function Toggle({ value, onChange }) {
  return (
    <button type="button" className="twk-toggle" data-on={value ? '1' : '0'}
            onClick={() => onChange(!value)}>
      <i />
    </button>
  );
}

function SegRadio({ value, options, onChange }) {
  const n = options.length;
  const idx = Math.max(0, options.indexOf(value));
  return (
    <div className="twk-seg" style={{ flex: 1 }}>
      <div className="twk-seg-thumb"
           style={{ left: `calc(2px + ${idx} * (100% - 4px) / ${n})`,
                    width: `calc((100% - 4px) / ${n})` }} />
      {options.map((o) => (
        <button key={o} type="button" onClick={() => onChange(o)}>{o}</button>
      ))}
    </div>
  );
}

function ColorChips({ value, options, onChange }) {
  return (
    <div className="twk-chips">
      {options.map((c) => (
        <button key={c} type="button" className="twk-chip"
                data-on={value === c ? '1' : '0'}
                style={{ background: c }} onClick={() => onChange(c)} />
      ))}
    </div>
  );
}

export default function TweaksPanel({ tweaks, setTweak }) {
  const [open, setOpen] = useState(false);
  const dragRef = useRef(null);
  const posRef = useRef({ right: 16, bottom: 64 });

  const onDragStart = (e) => {
    const panel = dragRef.current;
    if (!panel) return;
    const r = panel.getBoundingClientRect();
    const sx = e.clientX, sy = e.clientY;
    const startRight  = window.innerWidth  - r.right;
    const startBottom = window.innerHeight - r.bottom;
    const move = (ev) => {
      posRef.current = {
        right:  Math.max(8, startRight  - (ev.clientX - sx)),
        bottom: Math.max(8, startBottom - (ev.clientY - sy)),
      };
      panel.style.right  = posRef.current.right  + 'px';
      panel.style.bottom = posRef.current.bottom + 'px';
    };
    const up = () => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', up);
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
  };

  if (!open) {
    return (
      <button className="twk-open-btn" title="Tweaks" onClick={() => setOpen(true)}>⚙</button>
    );
  }

  return (
    <div ref={dragRef} className="twk-panel"
         style={{ right: posRef.current.right, bottom: posRef.current.bottom }}>
      <div className="twk-hd" onMouseDown={onDragStart}>
        <b>Tweaks</b>
        <button className="twk-x" onMouseDown={e => e.stopPropagation()}
                onClick={() => setOpen(false)}>✕</button>
      </div>
      <div className="twk-body">
        <div className="twk-sect">Look</div>
        <div className="twk-row">
          <span className="twk-lbl">Accent</span>
          <ColorChips value={tweaks.accent}
            options={['#3b6ea5', '#5a8f5a', '#c08a2e', '#2c2a27']}
            onChange={v => setTweak('accent', v)} />
        </div>
        <div className="twk-row">
          <span className="twk-lbl">Board tone</span>
          <SegRadio value={tweaks.theme} options={['warm', 'grey']}
            onChange={v => setTweak('theme', v)} />
        </div>
        <div className="twk-row">
          <span className="twk-lbl">Hand-drawn frames</span>
          <Toggle value={tweaks.sketch} onChange={v => setTweak('sketch', v)} />
        </div>
      </div>
    </div>
  );
}
