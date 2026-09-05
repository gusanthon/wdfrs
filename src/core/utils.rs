//! Utility functions for type conversion, signal generation, math approximations, and plotting.

use crate::core::wdf::WDFloat;
use rand::RngExt;

#[cfg(feature = "plot")]
use plotly::{
    common::{DashType, Line, Mode, Title},
    layout::{Axis, AxisType},
    Layout, Plot, Scatter,
};

/// Returns the short name of a type (without generic parameters or path).
pub fn type_of<T: ?Sized>() -> String {
    let full_type = std::any::type_name::<T>();
    full_type
        .split('<')
        .next()
        .unwrap_or(full_type)
        .split("::")
        .last()
        .unwrap_or(full_type)
        .into()
}

/// Helper to convert an f64 literal to a `WDFloat` type.
pub fn num_t<T: WDFloat>(num: f64) -> T {
    T::from_f64(num).unwrap()
}

/// Converts decibels (dB) to linear gain.
pub fn db_to_gain<T: WDFloat>(db: T) -> T {
    num_t::<T>(10.0).powf(db / num_t(20.0))
}

/// Converts a linear value to decibels.
pub fn gain_to_db<T: WDFloat>(lin: T) -> T {
    num_t::<T>(20.0) * lin.log10()
}

pub fn rand_wave<T: WDFloat>(n_samples: usize) -> Vec<T> {
    let mut rng = rand::rng();

    (0..n_samples)
        .map(|_| {
            let v = rng.random_range(-1.0..=1.0);
            T::from_f64(v).unwrap()
        })
        .collect()
}

/// Fast approximation of the Wright Omega function (Lambert W related).
///
/// Used for solving non-linear diode equations explicitly.
/// Ref: <https://www.dafx.de/paper-archive/2019/DAFx2019_paper_5.pdf>
pub fn omega_approx<T: WDFloat>(x: T) -> T {
    let x1 = num_t::<T>(-3.34145955276862);
    let x2 = num_t::<T>(8.0);
    let a = num_t::<T>(-1.3142931498778e-3);
    let b = num_t::<T>(4.775931364975583e-2);
    let c = num_t::<T>(3.631952663804445e-1);
    let d = num_t::<T>(6.313183464296682e-1);

    let y = if x < x1 {
        T::zero()
    } else if x < x2 {
        d + x * (c + x * (b + x * a))
    } else {
        x - x.ln()
    };

    y - (y - (x - y).exp()) / (y + T::one())
}

/// Plots an impulse response with sample/amplitude axis labels.
#[cfg(feature = "plot")]
pub fn plot_impulse_response_signal(signal: &[f64], title: &str) {
    let time_indices: Vec<f64> = (0..signal.len()).map(|x| x as f64).collect();

    let layout = Layout::new()
        .title(Title::with_text(title))
        .x_axis(Axis::new().title(Title::with_text("Sample")))
        .y_axis(Axis::new().title(Title::with_text("Amplitude")));
    let mut plot = Plot::new();
    let trace = Scatter::new(time_indices, signal.to_vec()).mode(Mode::Lines);
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot.show();
}

#[cfg(feature = "plot")]
fn log_freq_axis(fs: f64) -> Axis {
    Axis::new()
        .type_(AxisType::Log)
        .range(vec![20f64.log10(), (fs / 2.0).log10()])
        .title(Title::with_text("Frequency (Hz)"))
}

#[cfg(feature = "plot")]
fn mag_db_axis() -> Axis {
    Axis::new().title(Title::with_text("Magnitude (dB)"))
}

/// Plots a magnitude response with a logarithmic frequency x-axis in Hz.
#[cfg(feature = "plot")]
pub fn plot_magnitude_response(signal: &[f64], fs: f64, title: &str) {
    let n_samples = 2 * (signal.len() - 1);
    let freqs: Vec<f64> = (0..signal.len())
        .map(|k| k as f64 * fs / n_samples as f64)
        .collect();

    let layout = Layout::new()
        .title(Title::with_text(title))
        .x_axis(log_freq_axis(fs))
        .y_axis(mag_db_axis());
    let mut plot = Plot::new();
    let trace = Scatter::new(freqs, signal.to_vec()).mode(Mode::Lines);
    plot.add_trace(trace);
    plot.set_layout(layout);
    plot.show();
}

/// Plots multiple magnitude responses with labels and a logarithmic frequency x-axis in Hz.
#[cfg(feature = "plot")]
pub fn plot_magnitude_responses_labels(signals: &[&[f64]], labels: &[&str], fs: f64, title: &str) {
    let len = signals[0].len();

    assert!(signals.iter().all(|signal| signal.len() == len));

    let n_samples = 2 * (len - 1);
    let freqs: Vec<f64> = (0..len).map(|k| k as f64 * fs / n_samples as f64).collect();

    let dash_styles = [
        DashType::Solid,
        DashType::Dash,
        DashType::Dot,
        DashType::DashDot,
        DashType::LongDash,
        DashType::LongDashDot,
    ];

    let layout = Layout::new()
        .title(Title::with_text(title))
        .x_axis(log_freq_axis(fs))
        .y_axis(mag_db_axis());
    let mut plot = Plot::new();

    for (i, (signal, label)) in signals.iter().zip(labels).enumerate() {
        let line_style = Line::new().dash(dash_styles[i % dash_styles.len()].clone());

        let trace = Scatter::new(freqs.clone(), signal.to_vec())
            .name(label)
            .mode(Mode::Lines)
            .line(line_style);

        plot.add_trace(trace);
    }

    plot.set_layout(layout);
    plot.show();
}
