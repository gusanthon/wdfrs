use crate::core::{rtype::*, utils::num_t, wdf::*};
use wdf_macros::WDFCircuit;

#[derive(WDFCircuit)]
pub struct Baxandall<T: WDFloat, R: RootWDF<T>> {
    #[wdf_v_input]
    vin: ResVSHandle<T>,

    #[wdf_v_output]
    rl: ResHandle<T>,

    #[wdf_root]
    root: WDFRoot<R>,
}

const PT: f64 = 100e3;
const PB: f64 = 100e3;

pub type BaxandallCircuit<T: WDFloat> = Box<Baxandall<T, impl RootWDF<T>>>;

#[define_opaque(BaxandallCircuit)]
pub fn new_baxandall<T: WDFloat>(fs: T) -> BaxandallCircuit<T> {
    // Port A
    let p4 = Resistor::new(num_t(PT * 0.5)).parallel_with(Resistor::new(num_t(10e3)));

    let s4 = Capacitor::new(num_t(6.4e-9), fs).series_with(p4);

    // Port B
    let p5 = Resistor::new(num_t(PT * 0.5)).parallel_with(Resistor::new(num_t(1e3)));

    let s5 = Capacitor::new(num_t(64e-9), fs).series_with(p5);

    let p1 = Resistor::new(num_t(1e6)).id("rl").parallel_with(s5);

    // Port C
    let resc = Resistor::new(num_t(10e3));

    // Port D
    let p3 = Resistor::new(num_t(PB * 0.5)).parallel_with(Capacitor::new(num_t(220e-9), fs));
    let s3 = Resistor::new(num_t(1e3)).series_with(p3);

    // Port E
    let p2 = Resistor::new(num_t(PB * 0.5)).parallel_with(Capacitor::new(num_t(22e-9), fs));
    let s2 = Resistor::new(num_t(10e3)).series_with(p2);

    // Port F
    let vin = ResistiveVoltageSource::new(T::zero()).id("vin");
    let s1 = vin.series_with(Capacitor::new(num_t(1e-6), fs));

    let rtype = RootRType6Port::new(s4, p1, resc, s3, s2, s1, impedance_calc);

    Baxandall::from_root(rtype)
}

#[rustfmt::skip]
fn impedance_calc<T: WDFloat, P1, P2, P3, P4, P5, P6>(r: &RootRType6Port<T, P1, P2, P3, P4, P5, P6>)
where
    P1: WDF<T>,
    P2: WDF<T>,
    P3: WDF<T>,
    P4: WDF<T>,
    P5: WDF<T>,
    P6: WDF<T>,
{
    let [ra, rb, rc, rd, re, rf] = r.get_port_impedances();

    let one = T::one();
    let two = T::from_f64(2.0).unwrap();

    let scattering_matrix = [
        -two * ((rb + rc) * rd + (rb + rc + rd) * re + (rb + rc + rd) * rf) * ra
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf)
            + one,
        -two * (rc * rd + (rc + rd) * re + rc * rf) * ra
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        two * (rb * rd + rb * re + (rb + rd) * rf) * ra
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (rb * re - rc * rf) * ra
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        two * (rb * rd + (rb + rc + rd) * rf) * ra
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (rc * rd + (rb + rc + rd) * re) * ra
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (rc * rd + (rc + rd) * re + rc * rf) * rb
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * ((ra + rc) * rd + (ra + rc + rd) * re + (ra + rc + re) * rf) * rb
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf)
            + one,
        -two * (ra * rd + ra * re + (ra + re) * rf) * rb
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        two * (ra * re + (ra + rc + re) * rf) * rb
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (ra * rd - rc * rf) * rb
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * ((ra + rc) * rd + (rc + rd) * re) * rb
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        two * (rb * rd + rb * re + (rb + rd) * rf) * rc
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (ra * rd + ra * re + (ra + re) * rf) * rc
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * ((ra + rb) * rd + (ra + rb) * re + (ra + rb + rd + re) * rf) * rc
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf)
            + one,
        two * ((ra + rb) * re + (ra + re) * rf) * rc
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * ((ra + rb) * rd + (rb + rd) * rf) * rc
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (ra * rd - rb * re) * rc
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (rb * re - rc * rf) * rd
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        two * (ra * re + (ra + rc + re) * rf) * rd
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        two * ((ra + rb) * re + (ra + re) * rf) * rd
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (ra * rb + (ra + rb) * rc + (ra + rb) * re + (ra + rc + re) * rf) * rd
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf)
            + one,
        -two * (ra * rb + (ra + rb) * rc + rc * rf) * rd
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (ra * rb + (ra + rb) * rc + rb * re) * rd
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        two * (rb * rd + (rb + rc + rd) * rf) * re
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (ra * rd - rc * rf) * re
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * ((ra + rb) * rd + (rb + rd) * rf) * re
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (ra * rb + (ra + rb) * rc + rc * rf) * re
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (ra * rb + (ra + rb) * rc + (ra + rb) * rd + (rb + rc + rd) * rf) * re
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf)
            + one,
        -two * (ra * rb + (ra + rb) * rc + ra * rd) * re
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (rc * rd + (rb + rc + rd) * re) * rf
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * ((ra + rc) * rd + (rc + rd) * re) * rf
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (ra * rd - rb * re) * rf
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (ra * rb + (ra + rb) * rc + rb * re) * rf
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (ra * rb + (ra + rb) * rc + ra * rd) * rf
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf),
        -two * (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf
            / ((ra * rb + (ra + rb) * rc) * rd
                + (ra * rb + (ra + rb) * rc + (ra + rb) * rd) * re
                + (ra * rb + (ra + rb) * rc + (ra + rc) * rd + (rb + rc + rd) * re) * rf)
            + one,
    ];
    r.set_scattering_matrix(scattering_matrix);
}
