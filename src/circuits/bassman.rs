use crate::core::{circuit::*, rtype::RootRType6Port, utils::num_t, wdf::*};
use wdf_macros::WDFCircuit;

type S2Handle<T> = SerAdapHandle<T, Capacitor<T>, SeriesAdaptor<T, Resistor<T>, Resistor<T>>>;

#[derive(WDFCircuit)]
pub struct BassmanToneStack<T: WDFloat, R: RootWDF<T>> {
    res1m: ResHandle<T>,

    res3m: ResHandle<T>,

    s2: S2Handle<T>,

    vres: ResVSHandle<T>,

    res1p: ResHandle<T>,
    res2: ResHandle<T>,
    res3p: ResHandle<T>,

    #[wdf_root]
    root: WDFRoot<R>,
}

impl<T: WDFloat, R: RootWDF<T>> WDFCircuit<T> for BassmanToneStack<T, R> {
    fn process(&mut self, sample: T) -> T {
        self.vres.set_voltage(sample);
        self.root.compute();
        -(self.res1m.wave_to_voltage() + self.s2.wave_to_voltage() + self.res3m.wave_to_voltage())
    }
}

pub fn new_bassman_tone_stack<T: WDFloat>(
    fs: T,
    highpot: T,
    lowpot: T,
    midpot: T,
) -> Box<BassmanToneStack<T, impl RootWDF<T>>> {
    let cap1 = Capacitor::new(num_t(250e-12), fs);
    let cap2 = Capacitor::new(num_t(20e-9), fs);
    let cap3 = Capacitor::new(num_t(20e-9), fs);

    let res1p = Resistor::new(num_t(1.0)).id("res1p");
    let res1m = Resistor::new(num_t(1.0)).id("res1m");
    let res2 = Resistor::new(num_t(1.0)).id("res2");
    let res3m = Resistor::new(num_t(1.0)).id("res3m");
    let res3p = Resistor::new(num_t(1.0)).id("res3p");
    let res4 = Resistor::new(num_t(56e3));

    // Port A
    let vres = ResistiveVoltageSource::new(T::one()).id("vres");
    let s1 = vres.series_with(res3m);

    // Port B
    let s3 = res2.series_with(res3p);

    // Port C
    let s4 = res1p.series_with(res1m);
    let s2 = cap1.series_with(s4).id("s2");

    let root = RootRType6Port::new(s1, s3, s2, cap2, res4, cap3, impedance_calc);

    let mut circuit = BassmanToneStack::from_root(root);
    circuit.set_high(highpot);
    circuit.set_low(lowpot);
    circuit.set_mid(midpot);
    circuit
}

impl<T: WDFloat, R: RootWDF<T>> BassmanToneStack<T, R> {
    pub fn set_high(&mut self, highpot: T) {
        self.res1m.set_resistance(highpot * num_t(R1));
        self.res1p.set_resistance((T::one() - highpot) * num_t(R1));
        self.recalc_impedance();
    }

    pub fn set_low(&mut self, lowpot: T) {
        self.res2.set_resistance((T::one() - lowpot) * num_t(R2));
        self.recalc_impedance();
    }

    pub fn set_mid(&mut self, midpot: T) {
        self.res3m.set_resistance(midpot * num_t(R3));
        self.res3p.set_resistance((T::one() - midpot) * num_t(R3));
        self.recalc_impedance();
    }
}

const R1: f64 = 250e3;
const R2: f64 = 1e6;
const R3: f64 = 25e3;

#[rustfmt::skip]
fn impedance_calc<T, P1, P2, P3, P4, P5, P6>(r: &RootRType6Port<T, P1, P2, P3, P4, P5, P6>)
where
    T: WDFloat,
    P1: WDF<T>,
    P2: WDF<T>,
    P3: WDF<T>,
    P4: WDF<T>,
    P5: WDF<T>,
    P6: WDF<T>,
{
    let impedances = r.get_port_impedances();
    let [ra, rb, rc, rd, re, rf] = [
        impedances[0],
        impedances[1],
        impedances[2],
        impedances[3],
        impedances[4],
        impedances[5],
    ];

    let (ga, gb, gc, gd, ge, gf) = (
        T::one() / ra,
        T::one() / rb,
        T::one() / rc,
        T::one() / rd,
        T::one() / re,
        T::one() / rf,
    );

    // Common denominator
    let den = ga * gb * gd
        + ga * gb * ge
        + ga * gb * gf
        + ga * gc * gd
        + ga * gc * ge
        + ga * gc * gf
        + ga * gd * ge
        + ga * gd * gf
        + gb * gc * gd
        + gb * gc * ge
        + gb * gc * gf
        + gb * gd * ge
        + gb * ge * gf
        + gc * gd * gf
        + gc * ge * gf
        + gd * ge * gf;

    let s = num_t::<T>(2.0) / den;

    let m = [
        ra * s
            * (-ga * gb * gc * gd
                - ga * gb * gc * ge
                - ga * gb * gc * gf
                - ga * gb * gd * ge
                - ga * gb * ge * gf
                - ga * gc * gd * gf
                - ga * gc * ge * gf
                - ga * gd * ge * gf)
            + T::one(),
        ra * s * (ga * gb * gc * gd + ga * gb * gc * ge + ga * gb * gc * gf + ga * gb * gd * ge),
        ra * s * (ga * gb * gc * gd + ga * gb * gc * ge + ga * gb * gc * gf + ga * gc * gd * gf),
        ra * s * (ga * gb * gd * ge - ga * gc * gd * gf),
        ra * s * (-ga * gb * gd * ge - ga * gb * ge * gf - ga * gc * ge * gf - ga * gd * ge * gf),
        ra * s * (-ga * gb * ge * gf - ga * gc * gd * gf - ga * gc * ge * gf - ga * gd * ge * gf),
        rb * s * (ga * gb * gc * gd + ga * gb * gc * ge + ga * gb * gc * gf + ga * gb * gd * ge),
        rb * s
            * (-ga * gb * gc * gd
                - ga * gb * gc * ge
                - ga * gb * gc * gf
                - ga * gb * gd * ge
                - ga * gb * gd * gf
                - gb * gc * gd * gf
                - gb * gc * ge * gf
                - gb * gd * ge * gf)
            + T::one(),
        rb * s * (-ga * gb * gc * gd - ga * gb * gc * ge - ga * gb * gc * gf - gb * gc * ge * gf),
        rb * s * (-ga * gb * gd * ge - ga * gb * gd * gf - gb * gc * gd * gf - gb * gd * ge * gf),
        rb * s * (ga * gb * gd * ge - gb * gc * ge * gf),
        rb * s * (-ga * gb * gd * gf - gb * gc * gd * gf - gb * gc * ge * gf - gb * gd * ge * gf),
        rc * s * (ga * gb * gc * gd + ga * gb * gc * ge + ga * gb * gc * gf + ga * gc * gd * gf),
        rc * s * (-ga * gb * gc * gd - ga * gb * gc * ge - ga * gb * gc * gf - gb * gc * ge * gf),
        rc * s
            * (-ga * gb * gc * gd
                - ga * gb * gc * ge
                - ga * gb * gc * gf
                - ga * gc * gd * ge
                - ga * gc * gd * gf
                - gb * gc * gd * ge
                - gb * gc * ge * gf
                - gc * gd * ge * gf)
            + T::one(),
        rc * s * (ga * gc * gd * ge + ga * gc * gd * gf + gb * gc * gd * ge + gc * gd * ge * gf),
        rc * s * (-ga * gc * gd * ge - gb * gc * gd * ge - gb * gc * ge * gf - gc * gd * ge * gf),
        rc * s * (ga * gc * gd * gf - gb * gc * ge * gf),
        rd * s * (ga * gb * gd * ge - ga * gc * gd * gf),
        rd * s * (-ga * gb * gd * ge - ga * gb * gd * gf - gb * gc * gd * gf - gb * gd * ge * gf),
        rd * s * (ga * gc * gd * ge + ga * gc * gd * gf + gb * gc * gd * ge + gc * gd * ge * gf),
        rd * s
            * (-ga * gb * gd * ge
                - ga * gb * gd * gf
                - ga * gc * gd * ge
                - ga * gc * gd * gf
                - gb * gc * gd * ge
                - gb * gc * gd * gf
                - gb * gd * ge * gf
                - gc * gd * ge * gf)
            + T::one(),
        rd * s * (ga * gb * gd * ge + ga * gc * gd * ge + gb * gc * gd * ge + gc * gd * ge * gf),
        rd * s * (-ga * gb * gd * gf - ga * gc * gd * gf - gb * gc * gd * gf - gb * gd * ge * gf),
        re * s * (-ga * gb * gd * ge - ga * gb * ge * gf - ga * gc * ge * gf - ga * gd * ge * gf),
        re * s * (ga * gb * gd * ge - gb * gc * ge * gf),
        re * s * (-ga * gc * gd * ge - gb * gc * gd * ge - gb * gc * ge * gf - gc * gd * ge * gf),
        re * s * (ga * gb * gd * ge + ga * gc * gd * ge + gb * gc * gd * ge + gc * gd * ge * gf),
        re * s
            * (-ga * gb * gd * ge
                - ga * gb * ge * gf
                - ga * gc * gd * ge
                - ga * gc * ge * gf
                - ga * gd * ge * gf
                - gb * gc * gd * ge
                - gb * gc * ge * gf
                - gc * gd * ge * gf)
            + T::one(),
        re * s * (-ga * gb * ge * gf - ga * gc * ge * gf - ga * gd * ge * gf - gb * gc * ge * gf),
        rf * s * (-ga * gb * ge * gf - ga * gc * gd * gf - ga * gc * ge * gf - ga * gd * ge * gf),
        rf * s * (-ga * gb * gd * gf - gb * gc * gd * gf - gb * gc * ge * gf - gb * gd * ge * gf),
        rf * s * (ga * gc * gd * gf - gb * gc * ge * gf),
        rf * s * (-ga * gb * gd * gf - ga * gc * gd * gf - gb * gc * gd * gf - gb * gd * ge * gf),
        rf * s * (-ga * gb * ge * gf - ga * gc * ge * gf - ga * gd * ge * gf - gb * gc * ge * gf),
        rf * s
            * (-ga * gb * gd * gf
                - ga * gb * ge * gf
                - ga * gc * gd * gf
                - ga * gc * ge * gf
                - ga * gd * ge * gf
                - gb * gc * gd * gf
                - gb * gc * ge * gf
                - gb * gd * ge * gf)
            + T::one(),
    ];

    r.set_scattering_matrix(m);
}
