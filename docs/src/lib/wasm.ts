// Lazy-loads the phogbank wasm module on first use.
// If the pkg hasn't been built yet, all calls are no-ops and the play button
// shows an error. Build with:
//   wasm-pack build --target web --out-dir docs/src/pkg

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type RenderFn = (src: string, phrase_name: string, sample_rate: number) => any;

let renderFn: RenderFn | null = null;
let initPromise: Promise<boolean> | null = null;

export function initWasm(): Promise<boolean> {
  if (!initPromise) {
    initPromise = (async () => {
      try {
        // Dynamic import so missing pkg doesn't break the build.
        // eslint-disable-next-line @typescript-eslint/ban-ts-comment
        // @ts-ignore — generated at build time by wasm-pack
        const mod = await import('../pkg/phogbank.js');
        await mod.default();
        renderFn = mod.render_phrase_wasm as RenderFn;
        return true;
      } catch (e) {
        console.warn('phogbank wasm not loaded:', e);
        return false;
      }
    })();
  }
  return initPromise;
}

export function renderPhrase(
  src: string,
  phrase: string,
  sampleRate: number,
): Float32Array<ArrayBuffer> | null {
  if (!renderFn) return null;
  const raw = renderFn(src, phrase, sampleRate);
  // wasm-bindgen returns Float32Array<ArrayBufferLike>; copy to a concrete ArrayBuffer
  // so the Web Audio API (which requires Float32Array<ArrayBuffer>) accepts it.
  return new Float32Array(raw);
}
