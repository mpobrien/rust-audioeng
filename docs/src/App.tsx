import { useState, type ReactNode } from 'react';
import { Examples } from './components/Examples';
import { ThemeSwitcher } from './components/ThemeSwitcher';
import { IconCrab, IconHeadphones, IconMidi } from './components/Icons';

interface Principle {
  glyph: ReactNode;
  title: string;
  short?: string;
}

const PRINCIPLES: Principle[] = [
  { glyph: '#',                title: 'code is the score',     short: 'one file, version-controllable' },
  { glyph: <IconCrab />,       title: 'extensible with rust',  short: 'bring-your-own effects and synths in .rs' },
  { glyph: <IconHeadphones />, title: 'jam-friendly',          short: 'named scenes, tap-eval, live undo' },
  { glyph: <IconMidi />,       title: 'midi & osc',            short: 'first-class, bidirectional' },
];

const INSTALL_ONELINER = 'curl -sSf https://phog.bank/install.sh | sh';

function CopyCmd({ cmd }: { cmd: string }) {
  const [done, setDone] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(cmd);
      setDone(true);
      setTimeout(() => setDone(false), 1400);
    } catch { /* clipboard blocked */ }
  };
  return (
    <div className="cmd-box">
      <span className="sigil">$</span>
      <code>{cmd}</code>
      <button className={'copy' + (done ? ' done' : '')} onClick={copy}>
        {done ? '✓ copied' : 'copy'}
      </button>
    </div>
  );
}

export default function App() {
  return (
    <div className="page">
      {/* nav */}
      <nav className="nav">
        <a href="#" className="brand">
          <span className="brand-mark" />
          phogbank
        </a>
        <span className="version">v0.1.0</span>
        <span className="spacer" />
        <ThemeSwitcher />
        <a href="#examples">examples</a>
        <a href="#install">install</a>
        <a href="#docs">docs</a>
        <a href="#repo">github</a>
      </nav>

      {/* hero */}
      <header className="hero">
        <div className="hero-left">
          <div className="meta">
            <span className="dot" />
            <span>a programming language for sound &amp; pattern</span>
          </div>
          <h1 className="wordmark">
            phogbank<span className="blink">&nbsp;</span>
          </h1>
          <p className="tagline">
            <strong>Build synths, patterns, sequences, or entire tracks with code.</strong>
          </p>
          <p className="hero-meta">v0.0.1 · macOS / linux / windows · MIT</p>
          <div className="cta-row">
            <a href="#install" className="btn">
              install <span className="arrow">→</span>
            </a>
            <a href="#docs" className="btn ghost">docs</a>
          </div>
        </div>
        <div className="hero-right">
          <ul className="hero-list">
            {PRINCIPLES.map((p) => (
              <li key={p.title}>
                <span className="gl">{p.glyph}</span>
                <span><b>{p.title}</b>{p.short ? <> — {p.short}</> : null}</span>
              </li>
            ))}
          </ul>
        </div>
      </header>

      {/* examples */}
      <section className="section" id="examples">
        <div className="sec-hd">
          <span className="idx">§ 01</span>
          <h2>examples</h2>
          <span className="rule" />
          <span className="note">copy · paste · evaluate</span>
        </div>
        <Examples />
      </section>

      {/* compact coda — install */}
      <div className="coda" id="install">
        <div className="left">
          <span className="lbl">install · macOS · linux</span>
          <CopyCmd cmd={INSTALL_ONELINER} />
        </div>
      </div>

      {/* slim footer */}
      <footer className="foot">
        <span className="brand-mini">
          <span className="brand-mark" />
          <b>phogbank</b>
          <span style={{ color: 'var(--ink-faint)', marginLeft: 6 }}>
            v0.7.2 · MIT · © 2026
          </span>
        </span>
        <span className="links">
          <a href="#">github</a>
          <a href="#">discord</a>
          <a href="#">changelog</a>
          <a href="#">issues</a>
        </span>
      </footer>
    </div>
  );
}
