# wdfrs

[![CI](https://github.com/gusanthon/wdfrs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/gusanthon/wdfrs/actions/workflows/ci.yml?query=branch%3Amain)

A Rust library for real-time analog circuit modeling using Wave Digital Filters.

## Requirements

wdfrs requires a **nightly** Rust toolchain. It uses `type_alias_impl_trait` (TAIT) so that
circuit constructor return types can be written as opaque type aliases instead of spelling out the
full recursive generic tree:

```rust
// Without TAIT you'd need something like:
pub type RCLowPassCircuit<T> = Box<RCLowPass<T, IdealVoltageSource<T, SeriesAdaptor<T, Resistor<T>, Capacitor<T>>>>>
// which can quickly become huge

// With TAIT:
pub type RCLowPassCircuit<T: WDFloat> = Box<RCLowPass<T, impl RootWDF<T>>>;
```

A `rust-toolchain.toml` is included, so `cargo` will pick up the correct nightly automatically. It
resolves from the working directory rather than the manifest, so run cargo from the crate root,
`cargo build --manifest-path …` from elsewhere selects a different toolchain and fails.

TAIT reaches consumers too, since the aliases are public API: a crate that writes one of its own,
or that exports a circuit whose root is left to `_` through `export_wdf!`, needs
`#![feature(type_alias_impl_trait)]` in its own crate root and a nightly compatible with this
repo's pin.

## Components

- `Resistor`, `Capacitor`, `Inductor`
- `IdealVoltageSource`, `IdealCurrentSource`, `ResistiveVoltageSource`
- `SeriesAdaptor`, `ParallelAdaptor`, `PolarityInverter`
- `RType4Port` through `RType8Port`, `RootRType4Port` through `RootRType6Port`,
  with `define_rtype!` / `define_root_rtype!` for other port counts
- `ResistorCapacitorSeries`, `ResistorCapacitorParallel`
- `Diode`, `DiodePair` (Lambert W approximation)
- Fluent builder API: `.series_with()`, `.parallel_with()`, `.id()`
- `WDFHandle` / type aliases (`CapHandle`, `ResHandle`, ...) for runtime parameter changes
- Generic over `f32` and `f64` via `WDFloat`

## Quick Start

Build and drive the WDF tree directly:

```rust
use wdfrs::core::wdf::*;

let fs = 48e3;
let r = Resistor::new(1e3);
let c = Capacitor::new(1e-6, fs);
let root = r.series_with(c).to_ideal_voltage_source();

root.set_root_source_voltage(1.0);
let v_out = root.next.p2.wave_to_voltage(); // voltage across the capacitor
```

For a reusable struct with runtime-variable parameters, derive `WDFCircuit`:

```rust
use wdfrs::core::{circuit::WDFCircuit, wdf::*};
use wdf_macros::WDFCircuit;

#[derive(WDFCircuit)]
pub struct RCLowPass<T: WDFloat, R: RootWDF<T>> {
    #[wdf_v_output]       // voltage output
    c1: CapHandle<T>,

    r1: ResHandle<T>,     // handle for runtime parameter changes

    #[wdf_v_input]        // voltage input
    #[wdf_root]           // root of wdf tree
    root: WDFRoot<R>,     // type alias for Pin<Box<R>>
}

const C: f64 = 1e-6;

impl<T: WDFloat,R: RootWDF<T>> RCLowPass<T, R> {
    pub fn set_cutoff(&mut self, cutoff: T) {
        let resistance = T::one() / (num_t(TAU * C) * cutoff);
        self.r1.set_resistance(resistance);
        self.recalc_impedance();
    }
}

pub type RCLowPassCircuit<T: WDFloat> = Box<RCLowPass<T, impl RootWDF<T>>>;

#[define_opaque(RCLowPassCircuit)]
pub fn new_rc_lowpass<T: WDFloat>(fs: T, cutoff: T) -> RCLowPassCircuit<T> {
    let resistance = T::one() / (num_t(TAU * C) * cutoff);
    let s1 = Resistor::new(resistance)
        .id("r1")
        .series_with(Capacitor::new(num_t(C), fs).id("c1"));

    let root = s1.to_ideal_voltage_source();
    RCLowPass::from_root(root)
}
```

## Analysis

With the `fft` feature, circuits get `get_magnitude_response` and `get_phase_response`. With `plot`, they also get `plot_magnitude_response`, `plot_magnitude_response_list`, `plot_impulse_response`, and `plot_transfer_curve` — useful for quickly visualizing a circuit's behavior during development.

## Safety

All WDF components use `UnsafeCell` for interior mutability — the tree is single-threaded, acyclic, and traversed in a fixed order, making this sound. CI runs `cargo miri test -p wdfrs --all-features`; locally: `cargo miri test -p wdfrs --all-features` (after `rustup component add miri`).

## C FFI

Use `export_wdf!` to expose a circuit to C, then `cargo wdffi` to build the staticlib and generate a header:

```rust
export_wdf! {
    circuit: DiodeClipper<f32, _>,
    prefix: dc,
    constructor: |fs, cutoff, gain| new_diode_clipper(fs, cutoff, gain),
    precision: f32,
    methods: [set_cutoff, set_input_gain]
}
```

```sh
cargo install --path path/to/wdfrs/cargo-wdffi  # requires cargo-expand and cbindgen
cargo wdffi myfilter.h
```

`circuit` is the type as Rust spells it, with `_` standing for the root the constructor picked —
`new_diode_clipper` returns `Box<DiodeClipper<T, impl RootWDF<T>>>`, and nothing can name that root,
so `_` holds its place. A circuit that names its root itself, like one owning several trees, has
nothing to stand in for and is written out in full, `circuit: MyCircuit<f32>`; a circuit with no
generics at all is a bare `circuit: MyCircuit`.

Only the `_` form needs `#![feature(type_alias_impl_trait)]` in the exporting crate. The other two
name a type the alias can spell outright.

## Performance

Benchmarked at 192 kHz over 100 seconds of audio on Apple Silicon. x real-time, higher is better. Reference numbers from [wdf-bakeoff](https://github.com/jatinchowdhury18/wdf-bakeoff) (Faust via [wdmodels.lib](https://faustlibraries.grame.fr/libs/wdmodels/), C++ via [chowdsp_wdf](https://github.com/Chowdhury-DSP/chowdsp_wdf)).

| Circuit | wdfrs | Faust | C++ |
|---|---|---|---|
| LPF2 | **441.02x** | 253.35x | 269.82x |
| DiodeClipper | **264.37x** | 34.35x | 37.04x |
| BassmanToneStack | 257.76 x | **393.55x** | 9.25x |
| Baxandall | **278.18x** | 219.94x | 7.98x |
| FF2 | **136.47x** | 79.96x | 32.40x |

To reproduce the wdfrs column:
```sh
cargo run --example bench --release
```

## License

See [LICENSE](LICENSE).
