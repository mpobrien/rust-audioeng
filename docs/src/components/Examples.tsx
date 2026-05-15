import { useState, useRef, type ReactNode } from 'react';
import { highlightPb } from '../lib/highlight';
import { initWasm, renderPhrase } from '../lib/wasm';

interface Example {
  id: string;
  name: string;
  /** Name of the top-level phrase to render (the phrase = ... declaration). */
  phrase: string;
  blurb: ReactNode;
  code: string;
}

const EXAMPLES: Example[] = [
  {
    id: 'melody',
    name: 'melody.phog',
    phrase: 'melody',
    blurb: <>A saw-wave bass through an LFO-modulated lowpass, then an ADSR amplitude envelope. Nodes chain with <b>|</b>; <code>freq</code> and <code>velocity</code> are implicit params — every <b>patch</b> is a function of the incoming note.</>,
    code: `bass = patch {
  osc { shape = saw, freq = freq }
    | lpf  { cutoff = osc { freq = 10, min = 200, max = 2000 }, q = 1.2 }
    | adsr { attack = 0.02, decay = 0.08, sustain = 0.6, release = 0.12 }
}

melody = phrase { dur=eighth, tempo=120, [c d eb f g ab bb c5] } | bass`,
  },
  {
    id: 'supersaw',
    name: 'supersaw.phog',
    phrase: 'melody',
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

melody = phrase { dur=eighth, tempo=120, [ c4 d4 eb4 f4 g4 ab4 bb4 c5 ] } | supersaw`,
  },
  {
    id: 'fm-bell',
    name: 'fm_bell.phog',
    phrase: 'chime',
    blurb: <>Phase modulation bell: the carrier's phase is driven by a second <b>osc</b>. <b>level</b> sets the peak phase deviation in cycles. The <b>depth</b> envelope decays fast — maximum modulation at the strike, fading to a pure sine — the defining character of a struck bell.</>,
    code: `bell = patch {
  osc {
    shape  = sine,
    freq   = osc {
      freq   = freq * 2.0,
      level  = 2.5,
      depth  = adsr { attack = 0.001, decay = 0.8, sustain = 0, release = 0.1 },
      offset = freq
    }
  }
    | adsr { attack = 0.005, decay = 1.8, sustain = 0.0, release = 0.3 }
}

chime = phrase { dur=quarter, tempo=80, [ c4 d4 eb4 f4 g4 ab4 bb4 c5 ] } | bell`,
  },
  {
    id: 'fm-depth',
    name: 'fm_depth_osc.phog',
    phrase: 'melody',
    blurb: <>Instead of an ADSR, a 2 Hz square wave drives the <b>depth</b> of phase modulation. The timbre cycles between rich PM sidebands and pure sine twice a second. <b>level = 3</b> means ±3 cycles of phase swing at peak depth.</>,
    code: `pulse = patch {
  osc {
    shape  = sine,
    freq   = osc {
      shape  = square,
      freq   = freq * 1.4,
      level  = 3,
      depth  = osc { freq = 2 },
      offset = freq
    }
  }
  | hpf   { cutoff = 1600 }
  | delay { time = 0.8, feedback = 0.1 }
    | adsr { attack = 0.01, decay = 4.2, sustain = 0, release = 0.1 }
}

melody = phrase { dur=half, tempo=60, [c4 d4 eb4 f4 g4 ab4 bb4 c5] } | pulse`,
  },
];

type Status = 'idle' | 'loading' | 'playing' | 'error';

interface Playing {
  ctx: AudioContext;
  node: AudioBufferSourceNode;
}

export function Examples() {
  const [active, setActive] = useState<string>(EXAMPLES[0].id);
  const [status, setStatus] = useState<Status>('idle');
  const playingRef = useRef<Playing | null>(null);

  const ex = EXAMPLES.find((e) => e.id === active) ?? EXAMPLES[0];

  function stopAudio() {
    if (playingRef.current) {
      try {
        playingRef.current.node.onended = null;
        playingRef.current.node.stop();
        playingRef.current.ctx.close();
      } catch { /* ignore */ }
      playingRef.current = null;
    }
    setStatus('idle');
  }

  function handleTab(id: string) {
    stopAudio();
    setActive(id);
  }

  async function handlePlay() {
    if (status === 'playing' || status === 'loading') {
      stopAudio();
      return;
    }

    setStatus('loading');

    const ok = await initWasm();
    if (!ok) {
      setStatus('error');
      return;
    }

    const samples = renderPhrase(ex.code, ex.phrase, 44100);
    if (!samples || samples.length === 0) {
      setStatus('error');
      return;
    }

    const ctx = new AudioContext({ sampleRate: 44100 });
    const buf = ctx.createBuffer(1, samples.length, 44100);
    buf.copyToChannel(samples, 0);

    const node = ctx.createBufferSource();
    node.buffer = buf;
    node.connect(ctx.destination);
    node.onended = () => {
      ctx.close();
      playingRef.current = null;
      setStatus('idle');
    };
    node.start();

    playingRef.current = { ctx, node };
    setStatus('playing');
  }

  const isOn = status === 'playing';
  const label =
    status === 'playing' ? '■ stop' :
      status === 'loading' ? '… loading' :
        '▸ run';
  const hint =
    status === 'playing' ? `playing ${ex.name}` :
      status === 'loading' ? 'rendering…' :
        status === 'error' ? 'render error — check console' :
          'press run to evaluate region';

  return (
    <div className="examples">
      <div className="tabs" role="tablist">
        {EXAMPLES.map((e, i) => (
          <button
            key={e.id}
            role="tab"
            aria-selected={e.id === active}
            className={'tab' + (e.id === active ? ' active' : '')}
            onClick={() => handleTab(e.id)}
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
            data-on={isOn ? '1' : '0'}
            disabled={status === 'loading'}
            onClick={handlePlay}
          >
            {label}
          </button>
          <span style={{ fontFamily: 'inherit' }}>{hint}</span>
        </div>
      </div>
    </div>
  );
}
