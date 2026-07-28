//! Non-root R-type adaptors, checked against an equivalent tree built from
//! native adaptors.
//!
//! Nothing in `src/circuits` instantiates a non-root R-type — they all use the
//! `RootRType*` variants, which scatter through a different code path — so this
//! is the only coverage `define_rtype!` gets.

use wdfrs::core::rtype::RType4Port;
use wdfrs::core::utils::num_t;
use wdfrs::core::wdf::*;

const FS: f64 = 48e3;
const R_SERIES: f64 = 2.2e3;
const R_A: f64 = 10e3;
const R_B: f64 = 4.7e3;
const C: f64 = 100e-9;

/// A 3-way parallel junction: port 0 faces the parent, ports 1–3 the children.
type Junction = RType4Port<f64, Resistor<f64>, Capacitor<f64>, Resistor<f64>>;

/// A parallel junction is adapted when the up port carries the children's
/// total conductance, which makes `S[up][up]` zero.
fn junction_impedance(r: &Junction) -> f64 {
    let g: f64 = r.get_port_impedances().iter().map(|rp| 1.0 / rp).sum();
    1.0 / g
}

/// The junction's scattering matrix: `S[i][j] = 2*g_j/G - delta_ij`.
fn set_parallel_scattering(j: &Junction) {
    let g = [
        1.0 / j.params().rp,
        1.0 / j.p1.params().rp,
        1.0 / j.p2.params().rp,
        1.0 / j.p3.params().rp,
    ];
    let g_total: f64 = g.iter().sum();

    let mut s = [0.0; 16];
    for row in 0..4 {
        for col in 0..4 {
            s[row * 4 + col] = 2.0 * g[col] / g_total - f64::from(row == col);
        }
    }
    j.set_scattering_matrix(s);
}

/// A series resistor feeding three parallel elements, the junction expressed
/// as an R-type. The series element is what makes the junction's up-going wave
/// observable: with the junction hung straight off the source, every element
/// would simply sit at the source voltage.
fn rtype_tree() -> IdealVoltageSource<f64, SeriesAdaptor<f64, Resistor<f64>, Junction>> {
    let junction = RType4Port::new(
        Resistor::new(R_A),
        Capacitor::new(C, FS),
        Resistor::new(R_B),
        0,
        junction_impedance,
    );
    set_parallel_scattering(&junction);

    IdealVoltageSource::new(SeriesAdaptor::new(Resistor::new(R_SERIES), junction))
}

type Nested =
    ParallelAdaptor<f64, Resistor<f64>, ParallelAdaptor<f64, Capacitor<f64>, Resistor<f64>>>;

/// The same circuit, as nested two-port parallel adaptors.
fn native_tree() -> IdealVoltageSource<f64, SeriesAdaptor<f64, Resistor<f64>, Nested>> {
    IdealVoltageSource::new(SeriesAdaptor::new(
        Resistor::new(R_SERIES),
        ParallelAdaptor::new(
            Resistor::new(R_A),
            ParallelAdaptor::new(Capacitor::new(C, FS), Resistor::new(R_B)),
        ),
    ))
}

/// The two trees are the same circuit, so they agree to rounding error.
///
/// Regression test: `get_reflected_wave` used to return the up port's `b` wave
/// as left by the *previous* sample's scatter, a one-sample lag that showed up
/// here as ~0.2 of absolute error.
#[test]
fn rtype_junction_matches_native_adaptors() {
    let rtype = rtype_tree();
    let native = native_tree();

    let mut max_err = 0.0_f64;
    for n in 0..512 {
        let v = (num_t::<f64>(n as f64) * std::f64::consts::TAU * 1e3 / FS).sin();

        rtype.set_voltage(v);
        rtype.compute();

        native.set_voltage(v);
        native.compute();

        let err = (rtype.next.p1.wave_to_voltage() - native.next.p1.wave_to_voltage()).abs();
        max_err = max_err.max(err);
    }

    assert!(max_err < 1e-12, "trees diverged by {max_err:e}");
}
