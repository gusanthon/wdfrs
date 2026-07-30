//! The C ABI `export_wdf!` generates, exercised from outside the crate that
//! defines the circuits — which is what a consumer does.
//!
//! All three spellings of `circuit:` are covered: a `_` for the root the
//! constructor picked, a fully applied type for a circuit that names its root
//! itself, and a bare name for a circuit with no generics at all.

#![feature(type_alias_impl_trait)]

use wdfrs::circuits::diode_clipper::{new_diode_clipper, DiodeClipper};
use wdfrs::core::circuit::WDFCircuit;
use wdfrs::core::utils::num_t;
use wdfrs::core::wdf::*;
use wdfrs::export_wdf;

// ── circuit: applied type ────────────────────────────────────────────────────

/// A circuit that owns a concrete root, so there is no root left to stand in
/// for and the type is written out in full: `circuit: Rc1<f32>`.
pub struct Rc1<T: WDFloat> {
    root: IdealVoltageSource<T, SeriesAdaptor<T, Resistor<T>, Capacitor<T>>>,
    gain: T,
}

impl<T: WDFloat> Rc1<T> {
    pub fn set_gain(&mut self, gain: T) {
        self.gain = gain;
    }
}

impl<T: WDFloat> WDFCircuit<T> for Rc1<T> {
    fn process(&mut self, sample: T) -> T {
        self.root.set_voltage(sample * self.gain);
        self.root.compute();
        self.root.next.p2.wave_to_voltage()
    }
}

pub fn new_rc1<T: WDFloat>(fs: T) -> Box<Rc1<T>> {
    Box::new(Rc1 {
        root: IdealVoltageSource::new(SeriesAdaptor::new(
            Resistor::new(num_t(2.2e3)),
            Capacitor::new(num_t(100e-9), fs),
        )),
        gain: T::one(),
    })
}

export_wdf! {
    circuit: Rc1<f32>,
    prefix: rc1,
    constructor: |fs| new_rc1(fs),
    precision: f32,
    methods: [set_gain]
}

#[test]
fn applied_circuit_type() {
    unsafe {
        let c = rc1_f32_new(48e3);
        assert!(!c.is_null());

        rc1_f32_set_gain(c, 0.5);
        assert!(rc1_f32_process(c, 1.0).is_finite());

        let input = [1.0_f32; 8];
        let mut output = [0.0_f32; 8];
        rc1_f32_process_block(c, input.as_ptr(), output.as_mut_ptr(), input.len());
        assert!(output.iter().all(|v| v.is_finite()));

        // In place: hosts routinely pass one buffer as both input and output.
        // The C ABI must read each sample before overwriting its slot, and must
        // not form aliasing slices over the block, Miri guards the latter.
        let mut buf = [1.0_f32; 8];
        let p = buf.as_mut_ptr();
        rc1_f32_process_block(c, p as *const f32, p, buf.len());
        assert!(buf.iter().all(|v| v.is_finite()));

        rc1_f32_free(c);
    }
}

// ── circuit: root left to `_` ────────────────────────────────────────────────

// The README's example verbatim. The constructor hands back a root nothing can
// name, so `_` stands in for it, and this also checks that the impl the macro
// writes still applies through a `type … = impl RootWDF<T>` alias from another
// crate.
export_wdf! {
    circuit: DiodeClipper<f32, _>,
    prefix: dc,
    constructor: |fs, cutoff, gain| new_diode_clipper(fs, cutoff, gain),
    precision: f32,
    methods: [set_cutoff, set_input_gain]
}

#[test]
fn generic_root_circuit_type() {
    unsafe {
        let c = dc_f32_new(48e3, 1e3, 1.0);
        assert!(!c.is_null());

        dc_f32_set_cutoff(c, 2e3);
        dc_f32_set_input_gain(c, 2.0);
        assert!(dc_f32_process(c, 0.5).is_finite());

        dc_f32_free(c);
    }
}

// ── circuit: bare name ───────────────────────────────────────────────────────

// A series RC with no generic parameters at all — precision baked in. There is
// nothing to apply and no root to stand in for, so the bare name is the type.
pub struct RcBare {
    root: IdealVoltageSource<f32, SeriesAdaptor<f32, Resistor<f32>, Capacitor<f32>>>,
}

impl WDFCircuit<f32> for RcBare {
    fn process(&mut self, sample: f32) -> f32 {
        self.root.set_voltage(sample);
        self.root.compute();
        self.root.next.p2.wave_to_voltage()
    }
}

pub fn new_rc_bare(fs: f32) -> Box<RcBare> {
    Box::new(RcBare {
        root: IdealVoltageSource::new(SeriesAdaptor::new(
            Resistor::new(num_t(2.2e3)),
            Capacitor::new(num_t(100e-9), fs),
        )),
    })
}

export_wdf! {
    circuit: RcBare,
    prefix: rcbare,
    constructor: |fs| new_rc_bare(fs),
    precision: f32,
    methods: [],
}

#[test]
fn concrete_zero_generic_circuit_type() {
    unsafe {
        let c = rcbare_f32_new(48e3);
        assert!(!c.is_null());
        assert!(rcbare_f32_process(c, 1.0).is_finite());

        let mut buf = [1.0_f32; 4];
        let p = buf.as_mut_ptr();
        rcbare_f32_process_block(c, p as *const f32, p, buf.len());
        assert!(buf.iter().all(|v| v.is_finite()));

        rcbare_f32_free(c);
    }
}
