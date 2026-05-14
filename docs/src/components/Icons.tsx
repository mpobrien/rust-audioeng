// Small inline icons for the hero principles list.
// All use currentColor + var(--bg) so they recolor with the theme.

export const IconCrab = () => (
  <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" fill="currentColor">
    {/* body */}
    <path d="M4.2 13.5 Q4.2 10 12 10 Q19.8 10 19.8 13.5 L19.8 14.6 Q19.8 16.6 17.8 16.6 L6.2 16.6 Q4.2 16.6 4.2 14.6 Z" />
    {/* eye stalks + eyes */}
    <rect x="9.2" y="8.8" width="1.1" height="2.2" rx=".5" />
    <rect x="13.7" y="8.8" width="1.1" height="2.2" rx=".5" />
    <circle cx="9.75" cy="8.4" r="1.05" />
    <circle cx="14.25" cy="8.4" r="1.05" />
    <circle cx="9.75" cy="8.4" r=".4" fill="var(--bg)" />
    <circle cx="14.25" cy="8.4" r=".4" fill="var(--bg)" />
    {/* left claw + arm */}
    <path d="M2.6 9.2 L0.8 7.6 L2.6 6 L4.4 7.6 Z" />
    <path d="M4.6 8.4 L7 11" stroke="currentColor" strokeWidth="1.4"
          strokeLinecap="round" fill="none" />
    {/* right claw + arm */}
    <path d="M21.4 9.2 L23.2 7.6 L21.4 6 L19.6 7.6 Z" />
    <path d="M19.4 8.4 L17 11" stroke="currentColor" strokeWidth="1.4"
          strokeLinecap="round" fill="none" />
    {/* legs */}
    <path d="M5.6 16.4 L3.8 18.2 M7.2 17 L6.4 19.2 M16.8 17 L17.6 19.2 M18.4 16.4 L20.2 18.2"
          stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" fill="none" />
  </svg>
);

export const IconMidi = () => (
  <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" fill="none"
       stroke="currentColor" strokeWidth="1.6">
    {/* connector ring */}
    <circle cx="12" cy="12" r="8.6" />
    {/* 5 pins in a DIN arc */}
    <circle cx="12"   cy="6.6"  r="1.1" fill="currentColor" stroke="none" />
    <circle cx="7.4"  cy="9.1"  r="1.1" fill="currentColor" stroke="none" />
    <circle cx="16.6" cy="9.1"  r="1.1" fill="currentColor" stroke="none" />
    <circle cx="6.6"  cy="13.2" r="1.1" fill="currentColor" stroke="none" />
    <circle cx="17.4" cy="13.2" r="1.1" fill="currentColor" stroke="none" />
    {/* notch slot at bottom */}
    <rect x="10" y="16.2" width="4" height="2.1" rx="1.05"
          fill="currentColor" stroke="none" />
  </svg>
);

export const IconHeadphones = () => (
  <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" fill="none"
       stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
    <path d="M3.2 15 C3.2 6 12 4.2 12 4.2 C12 4.2 20.8 6 20.8 15" />
    <rect x="1.8" y="13.6" width="4.8" height="6.8" rx="1.6"
          fill="currentColor" stroke="none" />
    <rect x="17.4" y="13.6" width="4.8" height="6.8" rx="1.6"
          fill="currentColor" stroke="none" />
  </svg>
);
