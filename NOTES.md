# Future Enhancements

## Reduce dynamic dispatch

`Box<dyn SampleSource>` is used throughout the graph, which means vtable lookups on every sample, poor branch prediction, recursive heap traversal, and heap fragmentation from many small allocations scattered across memory.

Target direction:
- **Arena-based node storage** — allocate all nodes into a single contiguous slab; the graph holds indices, not pointers.
- **Indices instead of trait objects** — nodes reference each other by index into the arena; no vtable, no indirection.
- **Compiled execution plan** — at graph-compile time, flatten the tree into an ordered list of operations. At render time, iterate the list linearly.
- **Topologically sorted buffers** — each node writes into a pre-allocated slot; downstream nodes read from it. All audio data stays in a small, hot region of memory.

## Better graph execution model

The current model is recursive pull-based: a node asks its child for samples, which asks its child, and so on down the tree. This is elegant and composable but creates problems at scale:

- **Cycles** — impossible to represent; feedback paths (delay, reverb, FM synthesis) are blocked.
- **SIMD** — per-sample virtual dispatch prevents auto-vectorization.
- **Cache locality** — deep call stacks thrash the instruction cache; data is spread across the heap.
- **Shared subgraphs** — a node with multiple consumers gets evaluated multiple times.
- **Parallelism** — the call stack is implicitly serial; no opportunity for multi-core execution.

Target direction:
- **Explicit DSP graph** — nodes declared with explicit input/output edges, not embedded child pointers.
- **Per-node buffers** — each node owns a fixed-size output buffer (e.g. 128 samples). Processing a node means filling that buffer from its inputs' buffers.
- **Processing passes** — the execution plan is a flat, topologically sorted list of node IDs. Each render call iterates the list once, in order, with no recursion.

## MIDI support

Accept MIDI note on/off, CC, pitch bend, and clock messages as first-class inputs. Map MIDI events to gate triggers, frequency, velocity, and parameter values. Goal: a `Voice` can be driven directly from a MIDI stream without any glue code in the caller.

## Block-based processing

Per-sample processing with function call overhead per sample is expensive. Move toward a block-oriented interface:

```rust
fn process(&mut self, output: &mut [f32]);
```

All nodes operate on fixed-size blocks (e.g. 64 or 128 samples). This enables:
- SIMD-friendly inner loops with no per-sample virtual calls.
- Better compiler optimization (auto-vectorization, loop unrolling).
- Predictable latency budgeting.

The block size should be a compile-time constant or a global config, not a per-call parameter.

## User-defined synths and effects (plugin system)

Allow users to write custom `SampleSource` implementations in Rust and load them at runtime without recompiling phogbank. The website already hints at this with `~/.phog/lib/drive.rs`.

**Recommended approach: dynamic shared libraries via `libloading`**

Users write a Rust crate that exports a stable C ABI entry point:

```rust
#[no_mangle]
pub extern "C" fn phog_create(sample_rate: u32) -> *mut PluginVtable { ... }
```

`PluginVtable` is a plain C struct (defined in a `phog-plugin` crate) with function pointers for `process`, `reset`, and `destroy`. phogbank loads the `.dylib`/`.so` with `libloading`, calls `phog_create`, and wraps the result in a `Box<dyn SampleSource>` adapter. A file watcher hot-reloads the library when the user rebuilds.

**Key safety concerns to address:**
- Never pass `dyn Trait` fat pointers across the FFI boundary — use the C vtable struct instead.
- Wrap all calls into user code with `std::panic::catch_unwind` to prevent UB from panics crossing the FFI boundary.
- Version-stamp the `PluginVtable` struct so mismatched plugin/host ABIs fail fast with a clear error.

**Alternative: Wasm (future consideration)**
Compiling user code to `.wasm` and hosting it via Wasmtime gives full sandboxing, language-agnostic plugins, and trivial hot-reload. Worth revisiting once the plugin interface is stable.

## Parameter smoothing

Jumping a parameter value from one value to another in a single sample causes audible clicks (discontinuities in the waveform). All modulatable parameters — cutoff, gain, pitch — should be smoothed over a short window (typically 5–20ms) using a one-pole lowpass filter:

```
smoothed = smoothed + coeff * (target - smoothed)
```

`coeff` is derived from the desired smoothing time and sample rate. This should be transparent: the API accepts a target value, the smoother handles the ramp internally.
