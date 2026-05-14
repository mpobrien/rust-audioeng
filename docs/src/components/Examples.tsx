import { useState, type ReactNode } from 'react';
import { highlightPb } from '../lib/highlight';

interface Example {
  id: string;
  name: string;
  blurb: ReactNode;
  code: string;
}

const EXAMPLES: Example[] = [
  {
    id: 'melody',
    name: 'melody.patch',
    blurb: <>A saw-wave bass through an LFO-modulated lowpass, then an ADSR amplitude envelope. Nodes chain with <b>|</b>; <code>freq</code> and <code>velocity</code> are implicit params — every <b>patch</b> is a function of the incoming note.</>,
    code: `bass = patch {
  osc { shape = saw, freq = freq }
    | lpf  { cutoff = osc { freq = 10, min = 200, max = 2000 }, q = 1.2 }
    | adsr { attack = 0.02, decay = 0.08, sustain = 0.6, release = 0.12 }
}

melody = phrase { dur=eighth, tempo=120, [c d# c d# c d] } | bass`,
  },
  {
    id: 'supersaw',
    name: 'supersaw.patch',
    blurb: <>A 4-voice polyphonic supersaw. Two detuned oscillators are combined with <b>mix</b>, filtered, and gated through an ADSR. Named bindings (<code>osc1</code>, <code>body</code>) let you build up the graph incrementally before piping to <code>out</code>.</>,
    code: `space = effect {
  delay { time = 0.375, feedback = 0.45, mix = 0.35 }
}

supersaw = patch {
  voices = poly 4
  osc1 = osc { shape = saw, freq = freq }
  osc2 = osc { shape = saw, freq = freq * 2.4983 }
  body = mix { osc1, osc2 }
       | lpf { cutoff = 2000, q = 0.8 }
       | adsr { attack = 0.05, decay = 0.1, sustain = 0.8, release = 0.4 }
  out = body * velocity
}

melody = phrase { dur=eighth, tempo=120, [c d# c d# c d] } | supersaw`,
  },
  {
    id: 'fm-bell',
    name: 'fm_bell.patch',
    blurb: <>Classic FM bell: the carrier frequency comes from a second <b>osc</b>. The <b>depth</b> envelope decays fast — deep modulation at the strike, fading to a pure sine — which is what gives a struck bell its bright attack and clean ring.</>,
    code: `bell = patch {
  osc {
    shape  = sine,
    freq   = osc {
      freq      = freq * 9.4,
      deviation = 400,
      depth     = adsr { attack = 0.1, decay = 0.1, sustain = 1, release = 0.5 },
      offset    = freq
    }
  }
    | adsr { attack = 0.2, decay = 0.5, sustain = 0.0, release = 0.1 }
}

chime = phrase { dur=quarter, tempo=80, [c5 e5 g5 c6 _ g5 e5 c5] } | bell`,
  },
  {
    id: 'fm-depth',
    name: 'fm_depth_osc.patch',
    blurb: <>Instead of an ADSR, a 2 Hz square wave drives the <b>depth</b> of FM modulation. The timbre cycles between rich sidebands and pure sine twice a second. A highpass and delay push the texture further.</>,
    code: `pulse = patch {
  osc {
    shape  = sine,
    freq   = osc {
      shape     = square,
      freq      = freq * 1.4,
      deviation = 300,
      depth     = osc { freq = 2 },
      offset    = freq
    }
  }
  | hpf   { cutoff = 1600 }
  | delay { time = 0.8, feedback = 0.1 }
    | adsr { attack = 0.01, decay = 4.2, sustain = 0, release = 0.1 }
}

melody = phrase { dur=whole, tempo=60, [c4 g4] } | pulse`,
  },
];

export function Examples() {
  const [active, setActive] = useState<string>('drums');
  const [running, setRunning] = useState<string | null>(null);
  const ex = EXAMPLES.find((e) => e.id === active) ?? EXAMPLES[0];

  const toggleRun = () => {
    setRunning((cur) => (cur === active ? null : active));
  };

  return (
    <div className="examples">
      <div className="tabs" role="tablist">
        {EXAMPLES.map((e, i) => (
          <button
            key={e.id}
            role="tab"
            aria-selected={e.id === active}
            className={'tab' + (e.id === active ? ' active' : '')}
            onClick={() => setActive(e.id)}
          >
            <span className="tn">{String(i + 1).padStart(2, '0')}</span>
            <span>{e.name}</span>
          </button>
        ))}
      </div>
      <div className="tab-body">
        <p className="blurb">{ex.blurb}</p>
        <pre className="code code-wrap">{highlightPb(ex.code)}</pre>
        <div className="play-row">
          <button
            className="play"
            data-on={running === active ? '1' : '0'}
            onClick={toggleRun}
          >
            {running === active ? '■ stop' : '▸ run'}
          </button>
          <span style={{ fontFamily: 'inherit' }}>
            {running === active
              ? `evaluating ${ex.name} · live`
              : 'press run to evaluate region'}
          </span>
        </div>
      </div>
    </div>
  );
}
