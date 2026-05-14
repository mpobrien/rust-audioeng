import { useEffect, useState } from 'react';

export type ThemeName = 'paper' | 'ink' | 'neon' | 'pop' | 'slate';

const THEMES: { name: ThemeName; swatch: string; ring: string }[] = [
  { name: 'paper', swatch: '#f4f1ea', ring: '#ff5722' },
  { name: 'ink',   swatch: '#0a0a0a', ring: '#39ff14' },
  { name: 'neon',  swatch: '#1e104e', ring: '#ff653f' },
  { name: 'pop',   swatch: '#f6f7d7', ring: '#ff165d' },
  { name: 'slate', swatch: '#dfcdc3', ring: '#a26553' },
];

const STORAGE_KEY = 'phogbank.theme';

function readStoredTheme(): ThemeName {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    if (v && THEMES.some((t) => t.name === v)) return v as ThemeName;
  } catch { /* localStorage blocked */ }
  return 'paper';
}

export function ThemeSwitcher() {
  const [theme, setTheme] = useState<ThemeName>(() => readStoredTheme());

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
    try { localStorage.setItem(STORAGE_KEY, theme); } catch { /* noop */ }
  }, [theme]);

  return (
    <div className="theme-switcher" role="radiogroup" aria-label="Color theme">
      {THEMES.map((t) => (
        <button
          key={t.name}
          type="button"
          role="radio"
          aria-checked={t.name === theme}
          title={t.name}
          className={'theme-swatch' + (t.name === theme ? ' active' : '')}
          onClick={() => setTheme(t.name)}
          style={{ background: t.swatch }}
        >
          <span style={{ background: t.ring }} />
        </button>
      ))}
    </div>
  );
}
