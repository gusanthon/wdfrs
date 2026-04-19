use crate::core::{utils::num_t, wdf::*};
use wdf_macros::WDFCircuit;

#[derive(WDFCircuit)]
pub struct LPF2<T: WDFloat, R: RootWDF<T>> {
    #[wdf_v_output]
    c1: CapHandle<T>,

    #[wdf_v_input]
    #[wdf_root]
    root: WDFRoot<R>,
}

pub type LPF2Circuit<T: WDFloat> = Box<LPF2<T, impl RootWDF<T>>>;

#[define_opaque(LPF2Circuit)]
pub fn new_lpf2<T: WDFloat>(fs: T) -> LPF2Circuit<T> {
    let s1 = Resistor::new(num_t(10e3))
        .id("r1")
        .series_with(Capacitor::new(num_t(1e-6), fs).id("c1"));

    let p1 = Capacitor::new(num_t(1e-6), fs).id("c2").parallel_with(s1);

    let s2 = Resistor::new(num_t(10e3)).id("r2").series_with(p1);

    let root = IdealVoltageSource::new(s2);

    LPF2::from_root(root)
}
