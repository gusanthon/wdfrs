//! Demonstrates the four plot methods available on any [`WDFCircuit`] when
//! the `plot` feature is enabled (on by default).
//!
//! Each call opens an interactive Plotly chart in your default browser.
//!
//! ```sh
//! cargo run --example plot
//! ```
use wdfrs::{
    circuits::diode_clipper::{new_diode_clipper, DiodeClipper},
    WDFCircuit,
};

fn main() {
    let fs = 48e3;
    let n_fft = 8192;

    // 1. plot_magnitude_response - frequency response of a single circuit.
    let mut dc = new_diode_clipper(fs, 1e3_f64, 0.0_f64);
    dc.plot_magnitude_response(n_fft, fs);

    // 2. plot_magnitude_response_list — overlay responses for different
    //    parameter values. Here: diode clipper swept across cutoff frequencies.
    dc.plot_magnitude_response_list(
        n_fft,
        fs,
        DiodeClipper::set_cutoff,
        vec![500.0, 1e3, 5e3, 10e3],
    );

    // 3. plot_impulse_response — time-domain impulse response.
    dc.plot_impulse_response(n_fft);

    // 4. plot_transfer_curve — DC input/output curve; reveals nonlinear
    //    saturation characteristic of the diode pair and inverted phase.
    dc.set_input_gain(12.0);
    dc.plot_transfer_curve((-1.0, 1.0), 1000, 200);
}
