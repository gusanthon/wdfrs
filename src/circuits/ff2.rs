use crate::core::{utils::*, wdf::*};
use wdf_macros::WDFCircuit;

#[derive(WDFCircuit)]
pub struct FF2<T: WDFloat, R: RootWDF<T>> {
    #[wdf_v_output]
    r16: ResHandle<T>,

    #[wdf_v_input]
    #[wdf_root]
    root: WDFRoot<R>,
}

pub fn new_ff2<T: WDFloat>(fs: T) -> Box<FF2<T, impl RootWDF<T>>> {
    let s1 = Resistor::new(num_t(12000.0)).series_with(Capacitor::new(num_t(27e-9), fs));
    let p1 = s1.parallel_with(Resistor::new(num_t(27e3)));

    let s2 = Resistor::new(num_t(22e3)).series_with(Capacitor::new(num_t(2.2e-9), fs));
    let s3 = s2.series_with(Resistor::new(num_t(47e3)).id("r16"));
    let p2 = s3.parallel_with(p1);

    let p3 = p2.parallel_with(Resistor::new(num_t(50e3)));
    let s4 = p3.series_with(Resistor::new(num_t(50e3)));

    let s5 = Resistor::new(num_t(1e3)).series_with(Capacitor::new(num_t(390e-9), fs));
    let p4 = s4.parallel_with(s5);
    let p5 = p4.parallel_with(Resistor::new(num_t(1500.0)));
    let s6 = p5.series_with(ResistiveVoltageSource::new(T::zero()));

    let p6 = Resistor::new(num_t(4100.0)).parallel_with(Capacitor::new(num_t(68e-9), fs));
    let pi = PolarityInverter::new(p6.series_with(s6));

    FF2::from_root(IdealVoltageSource::new(pi))
}
