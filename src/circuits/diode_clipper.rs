use crate::core::{circuit::*, utils::*, wdf::*};
use std::f64::consts::TAU;
use wdf_macros::WDFCircuit;

#[derive(WDFCircuit)]
pub struct DiodeClipper<T: WDFloat, R: RootWDF<T>> {
    input_gain: T,

    rvs: ResVSHandle<T>,
    c1: CapHandle<T>,

    #[wdf_root]
    root: WDFRoot<R>,
}

pub type DiodeClipperCircuit<T: WDFloat> = Box<DiodeClipper<T, impl RootWDF<T>>>;

#[define_opaque(DiodeClipperCircuit)]
pub fn new_diode_clipper<T: WDFloat>(
    fs: T,
    cutoff_hz: T,
    input_gain_db: T,
) -> DiodeClipperCircuit<T> {
    let res = r_from_fc(cutoff_hz);

    let rvs = ResistiveVoltageSource::new(res).id("rvs");

    let p1 = rvs.parallel_with(Capacitor::new(num_t(47e-9), fs).id("c1"));

    let root = DiodePair::new(p1, num_t(2.52e-9));
    root.set_diode_params(num_t(2.52e-9), num_t(25.85e-3));

    let mut dc = DiodeClipper::from_root(root);
    dc.set_input_gain(input_gain_db);

    dc
}

impl<T: WDFloat, R: RootWDF<T>> WDFCircuit<T> for DiodeClipper<T, R> {
    fn process(&mut self, sample: T) -> T {
        self.rvs.set_voltage(sample * self.input_gain);
        self.root.compute();
        -self.c1.wave_to_voltage()
    }
}

impl<T: WDFloat, R: RootWDF<T>> DiodeClipper<T, R> {
    pub fn set_input_gain(&mut self, input_gain_db: T) {
        self.input_gain = db_to_gain(input_gain_db)
    }

    pub fn set_cutoff(&mut self, cutoff_hz: T) {
        let res = r_from_fc(cutoff_hz);
        self.rvs.set_resistance(res);
        self.recalc_impedance();
    }
}

fn r_from_fc<T: WDFloat>(cutoff_hz: T) -> T {
    T::one() / (num_t::<T>(TAU * 47e-9) * cutoff_hz)
}
