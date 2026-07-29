//! What `export_wdf!` does *not* require of the crate it expands in.
//!
//! Deliberately spare where [`export_wdf`](../export_wdf.rs) is generous: this
//! file has no `#![feature(type_alias_impl_trait)]` and no `use` of
//! `WDFCircuit`. The trait is named in full where the circuit implements it, so
//! nothing the macro expands to can be leaning on an import.
//!
//! Both omissions are load-bearing, and each fails on its own:
//!
//! - without the gate, an opaque `type … = impl Trait` alias in the expansion is
//!   E0658 — so this pins the alias staying concrete when `circuit:` already
//!   names its root;
//! - without the import, an unqualified `f.process(input)` in the expansion is
//!   E0599 on `__rc1_f32__Circuit`, a type the consumer never wrote.

use wdfrs::core::utils::num_t;
use wdfrs::core::wdf::*;
use wdfrs::export_wdf;

/// A circuit owning a concrete root, so `circuit:` takes the applied form.
pub struct Rc1<T: WDFloat> {
    root: IdealVoltageSource<T, SeriesAdaptor<T, Resistor<T>, Capacitor<T>>>,
    gain: T,
}

impl<T: WDFloat> Rc1<T> {
    pub fn set_gain(&mut self, gain: T) {
        self.gain = gain;
    }
}

impl<T: WDFloat> wdfrs::core::circuit::WDFCircuit<T> for Rc1<T> {
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
fn exports_without_tait_or_trait_import() {
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
