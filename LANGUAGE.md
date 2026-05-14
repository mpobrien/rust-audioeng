# Language Design

A two-layer music programming language. The **patch layer** describes what an
instrument sounds like (signal graph). The **pattern layer** describes what it
plays and when (musical sequences, chords, rhythms). Both layers are live-editable.

---

## Patches

A patch is a signal graph assigned to a variable. It describes a sound, not a
specific note — `freq` and `velocity` are implicit inputs injected by the
sequencer at note-on time.

```
bass = patch {
  osc { shape = saw, freq = freq }
    | lpf  { cutoff = 800, q = 1.5 }
    | adsr { attack = 0.02, decay = 0.1, sustain = 0.7, release = 0.15 }
}
```

Signal nodes take a `{ key = value }` table of named parameters. The `|`
operator chains them — each stage receives the output of the previous one as
its source.

### Polyphony

Polyphony is a field inside the patch block. It defaults to `mono` if omitted.

```
lead = patch {
  voices = mono          # one voice; new note cuts old one (default)
  ...
}

pad = patch {
  voices = poly 8        # up to 8 simultaneous voices
  ...
}

bass = patch {
  voices = mono legato   # monophonic with portamento (glide between notes)
  ...
}
```

When a `poly` patch runs out of voices, the oldest voice is stolen.

### Named nodes

Intermediate signals can be named and reused within the patch block:

```
supersaw = patch {
  voices = poly 4

  osc1 = osc { shape = saw, freq = freq }
  osc2 = osc { shape = saw, freq = freq * 1.006 }
  osc3 = osc { shape = saw, freq = freq * 0.994 }

  body = mix { osc1, osc2, osc3 }
       | lpf  { cutoff = 2000, q = 0.8 }
       | adsr { attack = 0.3, decay = 0.1, sustain = 0.8, release = 0.5 }

  out = body * velocity
}
```

`out` is the reserved name for the patch output. If the block contains only
one expression, it is the output automatically.

`mix { ... }` takes an array-like table of sources, normalising gains to
prevent clipping.

### Modulation

Any numeric parameter can accept a signal instead of a constant. The signal is
evaluated per-sample.

```
wub = patch {
  lfo1 = lfo { shape = sine, rate = 2.0 }

  osc { shape = saw, freq = freq }
    | lpf  { cutoff = lfo1 * 800 + 1200, q = 2.0 }
    | adsr { attack = 0.01, decay = 0.05, sustain = 0.9, release = 0.1 }
}
```

`lfo` is a free-running oscillator used as a modulation source. Its output
range is -1..1. Any node output can be used as a modulator.

---

## Effects

An effect is a named, reusable signal sub-chain. It transforms whatever signal
is piped into it and can be shared across multiple patches.

```
space = effect {
  delay  { time = 0.375, feedback = 0.45, mix = 0.4 }
    | reverb { size = 0.8, damp = 0.5, mix = 0.25 }
}
```

Use it in any patch by name in the pipe chain:

```
bass = patch {
  osc { shape = saw, freq = freq }
    | lpf  { cutoff = 800, q = 1.5 }
    | adsr { attack = 0.02, decay = 0.1, sustain = 0.7, release = 0.15 }
    | space
}

lead = patch {
  osc { shape = sine, freq = freq }
    | adsr { attack = 0.05, decay = 0.1, sustain = 0.8, release = 0.25 }
    | space
}
```

Both patches share the same `space` effect definition. Changing `space` live
updates every patch that uses it at the next bar boundary.

### Named nodes inside effects

Effects follow the same rules as patches — intermediate signals can be named:

```
shimmer = effect {
  wet  = reverb { size = 0.95, mix = 1.0 }
  high = wet | hpf { cutoff = 2000, q = 0.7 }
  out  = mix { wet * 0.4, high * 0.6 }
}
```

### Effects with ctrl values

Effects can reference `ctrl` values just like patches:

```
ctrl verb_mix  = cc { number = 30, range = 0.0..0.8 }
ctrl verb_size = cc { number = 31, range = 0.3..1.0 }

room = effect {
  reverb { size = verb_size, damp = 0.4, mix = verb_mix }
}
```

### Stacking effects

Effects can be composed with each other:

```
warmth = effect {
  lpf { cutoff = 3000, q = 0.6 }
    | amp { level = 1.1 }
}

full_chain = effect {
  warmth | space
}
```

### Placement in the signal chain

An effect can go anywhere in the pipe — before or after the envelope, or
between filter stages. Placement changes the character of the sound:

```
bass = patch {
  osc { shape = saw, freq = freq }
    | warmth                          # filter before envelope
    | adsr { attack = 0.01, decay = 0.08, sustain = 0.7, release = 0.12 }
    | space                           # reverb/delay after envelope tail
}
```

### Signal primitives

| Node | Parameters | Description |
|------|------------|-------------|
| `osc` | `shape`, `freq` | Oscillator. Shapes: `sine`, `saw`, `square`, `tri` |
| `lfo` | `shape`, `rate` | Free-running modulation oscillator |
| `lpf` | `cutoff`, `q` | Low-pass biquad filter |
| `hpf` | `cutoff`, `q` | High-pass biquad filter |
| `bpf` | `cutoff`, `q` | Band-pass biquad filter |
| `notch` | `cutoff`, `q` | Notch filter |
| `adsr` | `attack`, `decay`, `sustain`, `release` | Amplitude envelope, triggered at note-on |
| `delay` | `time`, `feedback`, `mix` | Delay line. `time` can be modulated |
| `mix` | `{ source, source, ... }` | Sum sources, normalised gains |
| `amp` | `level` | Scale amplitude. `level` can be a signal |
| `freq` | — | Implicit input: note frequency in Hz |
| `velocity` | — | Implicit input: note velocity, 0.0..1.0 |

---

## Patterns

A pattern is a named, looping musical sequence. It runs against the global
clock and declares which patch it plays through.

```
pattern bassline with bass {
  steps = "C2 _ Eb2 _ F2 _ G2 Ab2"
}
```

### Step notation

The `steps` string is a compact description of a rhythmic sequence. By
default, each token is one beat.

```
"C4 E4 G4 B4"           # four notes, one beat each
"C4 _ E4 _"             # underscore is a rest
"C4 [E4 G4] B4 _"       # brackets subdivide: E4 and G4 share one beat
"C4@2 E4 G4"            # @ sets duration in beats: C4 holds for 2 beats
"Cmaj7 _ _ _"           # chord name; spawns one voice per note (requires poly patch)
"1 3 5 7"               # scale degrees (requires a scale to be set)
"rand rand rand rand"   # random notes from the current scale
```

### Pattern options

```
pattern melody with lead {
  steps = "C4 E4 G4 B4 G4 E4 C4 _"

  gate = 0.8    # fraction of each step the note is held (0.0..1.0)
  vel  = 0.9    # default velocity for this pattern
  oct  = 0      # transpose by N octaves
  bars = 2      # loop length in bars (default: inferred from step count and meter)
}
```

### Multi-bar patterns

```
pattern chords with pad {
  bar 1: "Cmaj7 _ _ _"
  bar 2: "Cmaj7 _ _ _"
  bar 3: "Fmaj7 _ _ _"
  bar 4: "G7    _ _ _"
}
```

### Chord voicings

```
pattern harmony with pad {
  steps = "Cmaj7 Fmaj7 Amin7 G7"

  voicing = close    # notes packed into one octave (default)
  root    = C3       # base octave for voicing
}

# Inversions with slash notation
"Cmaj7/E Fmaj7/A Amin7/C G7/B"
```

Voicing options: `close`, `open`, `drop2`.

### Arpeggios

```
pattern arp with lead {
  arp Cmaj7 {
    order = up      # up, down, updown, random
    step  = 1/8     # note duration
    oct   = 2       # span N octaves
  }
}
```

---

## Musical vocabulary

### Notes

```
C4   D4   E4   F4   G4   A4   B4    # natural notes
C#4  Db4  F#3  Gb3  Bb5  A#5        # sharps and flats
```

Octave 4 is middle C. MIDI note 60 = C4 = 261.63 Hz.

### Chords

```
Cmaj   Cmin   Caug   Cdim            # triads
Cmaj7  Cmin7  C7     Cmin7b5         # seventh chords
Cmaj9  Cadd9  Csus2  Csus4           # extensions and suspensions
```

### Scales

```
scale = major
scale = minor
scale = dorian
scale = pentatonic
scale = blues
```

When a scale is set, scale degrees and `rand` in step notation are constrained
to that scale.

---

## MIDI

### Device setup

```
midi device "Arturia MiniLab"     # by name
midi device 0                     # by index

list midi devices                 # print available devices to console
```

Multiple devices can be declared and active simultaneously.

### Controller bindings

`ctrl` declares a named value driven by a hardware knob or slider. It can be
used anywhere a number is expected — patch parameters, pattern options, tempo.

```
ctrl filter_cutoff = cc { number = 74, range = 200..8000 }
ctrl resonance     = cc { number = 71, range = 0.5..4.0 }
ctrl master_vol    = cc { number = 7,  range = 0.0..1.0 }
```

MIDI CC values (0–127) are mapped linearly to the declared range by default.

**Mapping curves** — for controls where linear feels wrong:

```
ctrl filter_cutoff = cc { number = 74, range = 200..8000, curve = exp }
ctrl drive         = cc { number = 18, range = 0.0..1.0,  curve = log }
```

**Parameter smoothing** — prevents audible stepping when a knob is turned:

```
ctrl filter_cutoff = cc { number = 74, range = 200..8000, curve = exp, smooth = 30ms }
```

**Specific device:**

```
ctrl filter_cutoff = cc { number = 74, device = "Arturia MiniLab", range = 200..8000 }
```

### Using controllers in patches

`ctrl` values are signals and can modulate any parameter:

```
ctrl cutoff = cc { number = 74, range = 200..8000, curve = exp, smooth = 20ms }
ctrl reso   = cc { number = 71, range = 0.5..3.5,  smooth = 20ms }

synth = patch {
  voices = poly 4

  osc { shape = saw, freq = freq }
    | lpf  { cutoff = cutoff, q = reso }
    | adsr { attack = 0.02, decay = 0.1, sustain = 0.8, release = 0.15 }
    * velocity
}
```

A `ctrl` value can be combined with internal modulation:

```
animated = patch {
  voices = poly 4

  lfo1 = lfo { shape = sine, rate = 0.5 }

  osc { shape = saw, freq = freq }
    | lpf  { cutoff = cutoff + lfo1 * 300, q = reso }
    | adsr { attack = 0.02, decay = 0.1, sustain = 0.8, release = 0.15 }
}
```

### Using controllers in patterns

```
ctrl gate_ctrl = cc { number = 20, range = 0.3..1.0 }
ctrl vel_ctrl  = cc { number = 21, range = 0.4..1.0 }

pattern bassline with bass {
  steps = "C2 _ Eb2 _ F2 _ G2 _"
  gate  = gate_ctrl
  vel   = vel_ctrl
}
```

### Tempo control

```
ctrl bpm = cc { number = 1, range = 60..180, smooth = 500ms }
tempo = bpm
```

### Buttons and toggles

```
ctrl hold_btn  = cc { number = 20, mode = gate }    # 1.0 while held, 0.0 when released
ctrl latch_btn = cc { number = 21, mode = toggle }  # flips between 0.0 and 1.0 on each press
```

Buttons can gate patterns:

```
play bassline while hold_btn
play fills    when latch_btn
```

Or drive values inside patches:

```
ctrl overdrive_on = cc { number = 22, mode = toggle }

crunch = patch {
  body  = osc { shape = saw, freq = freq } | lpf { cutoff = 1200, q = 2.0 }
  clean = body | adsr { attack = 0.01, decay = 0.1, sustain = 0.8, release = 0.1 }
  dirty = body | drive { amount = 3.0 } | lpf { cutoff = 800, q = 1.0 } | adsr { attack = 0.01, decay = 0.1, sustain = 0.8, release = 0.1 }
  out   = mix { clean * (1.0 - overdrive_on), dirty * overdrive_on }
}
```

### MIDI keyboard input

```
midi keyboard to lead                  # all channels
midi keyboard channel 1 to bass        # specific channel
midi keyboard channel 2 to pad
```

Incoming note events are handled by the patch's voice manager, so polyphony
and voice stealing apply as normal.

### Tempo sync

```
midi sync from "Arturia KeyStep"
```

When sync is active, `tempo` is overridden and the clock follows the external
device.

---

## Composition

The composition section declares global settings and which patterns are playing.

```
tempo = 120
meter = 4/4
scale = C major

play bassline
play chords
play melody
```

### Play options

```
play melody  once          # play once, then stop
play arp     times 4       # play 4 times, then stop
play chords  at bar 3      # start at bar 3
play fills   every 4 bars  # play once every 4 bars
play bassline while hold_btn
play fills    when latch_btn
```

### Muting and soloing

```
mute chords
solo melody
unmute chords
stop bassline
```

---

## Live commands

These are typed into the REPL and take effect at the next bar boundary unless
prefixed with `!` for immediate effect.

```
tempo 140                  # change tempo (next bar)
! tempo 140                # change tempo immediately
stop all
play bassline
mute chords
solo lead
transpose melody +2        # shift up 2 semitones (next bar)
reverse melody
slow 2 melody              # half speed
fast 2 melody              # double speed
scale D minor
```

---

## A full example

```
# --- MIDI ---

midi device "Arturia MiniLab"

ctrl cutoff     = cc { number = 74, range = 200..8000, curve = exp, smooth = 20ms }
ctrl reso       = cc { number = 71, range = 0.5..3.5,  smooth = 20ms }
ctrl lfo_rate   = cc { number = 72, range = 0.1..8.0 }
ctrl master_vol = cc { number = 7,  range = 0.0..1.0 }

ctrl pad1 = cc { number = 20, mode = toggle }  # mute/unmute bassline
ctrl pad2 = cc { number = 21, mode = toggle }  # mute/unmute melody

midi keyboard channel 1 to lead

# --- Effects ---

ctrl verb_mix = cc { number = 30, range = 0.0..0.6, smooth = 50ms }

space = effect {
  delay  { time = 0.375, feedback = 0.4, mix = 0.35 }
    | reverb { size = 0.75, damp = 0.5, mix = verb_mix }
}

# --- Instruments ---

bass = patch {
  osc { shape = saw, freq = freq }
    | lpf  { cutoff = cutoff * 0.5, q = reso }
    | adsr { attack = 0.01, decay = 0.08, sustain = 0.7, release = 0.12 }
    * velocity * master_vol
}

lead = patch {
  lfo1 = lfo { shape = sine, rate = lfo_rate }

  osc { shape = sine, freq = freq + lfo1 * 4 }
    | lpf  { cutoff = cutoff, q = reso }
    | adsr { attack = 0.05, decay = 0.1, sustain = 0.8, release = 0.25 }
    * velocity * master_vol
    | space
}

pad = patch {
  voices = poly 6

  osc1 = osc { shape = sine, freq = freq }
  osc2 = osc { shape = sine, freq = freq * 2.003 }

  mix { osc1, osc2 }
    | lpf  { cutoff = lfo { shape = sine, rate = 0.2 } * 400 + 900, q = 0.6 }
    | adsr { attack = 0.8, decay = 0.2, sustain = 0.9, release = 1.5 }
    * velocity * master_vol
    | space
}

# --- Patterns ---

tempo = 120
meter = 4/4
scale = C minor

pattern bassline with bass {
  steps = "C2 _ Eb2 _ F2 _ G2 _"
  gate  = 0.85
}

pattern chords with pad {
  bar 1-2: "Cmin7 _ _ _"
  bar 3-4: "Fmin7 _ _ _"
  voicing = close
  root    = C3
}

pattern melody with lead {
  bar 1: "C4 Eb4 G4 _"
  bar 2: "Bb3 _ C4 Eb4"
  bar 3: "F4 _ Eb4 C4"
  bar 4: "G3 _ _ _"
  gate  = 0.7
}

# --- Start ---

play bassline when pad1
play chords
play melody   when pad2
```
