//! The C ABI `export_wdf!` generates, exercised from outside the crate that
//! defines the circuits — which is what a consumer does.
//!
//! Both accepted spellings of `circuit:` are covered: a bare name, for the
//! usual circuit generic over its root, and a name with its arguments applied,
//! for a circuit that names its root concretely.

#![feature(type_alias_impl_trait)]

use wdfrs::circuits::diode_clipper::{new_diode_clipper, DiodeClipper};
use wdfrs::core::circuit::WDFCircuit;
use wdfrs::core::utils::num_t;
use wdfrs::core::wdf::*;
use wdfrs::export_wdf;

// ── circuit: applied type ────────────────────────────────────────────────────

/// A circuit that owns a concrete root, so there is no `R` left to be generic
/// over and `circuit: Rc1<f32>` is the only way to name it.
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

        rc1_f32_free(c);
    }
}

// ── circuit: bare name ───────────────────────────────────────────────────────

// The README's example verbatim. The circuit is generic over its root, and the
// constructor hands back an opaque one, so this also checks that the impl the
// macro writes still applies through a `type … = impl RootWDF<T>` alias from
// another crate.
export_wdf! {
    circuit: DiodeClipper,
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
