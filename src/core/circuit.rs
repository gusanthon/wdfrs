//! Core traits and analysis methods for WDF circuits.

#[cfg(feature = "fft")]
use crate::core::utils::gain_to_db;
#[cfg(feature = "plot")]
use crate::core::utils::{
    plot_impulse_response_signal, plot_magnitude_response, plot_magnitude_responses_labels,
};
use crate::{
    core::bench::{bench_row_named, print_bench_table},
    core::utils::num_t,
    core::wdf::*,
};
#[cfg(feature = "plot")]
use plotly::{common::Title, layout::Axis, Layout, Plot, Scatter};
#[cfg(feature = "fft")]
use realfft::{num_complex::Complex, RealFftPlanner};

/// The main trait for a Wave Digital Filter circuit.
///
/// It provides methods for real-time signal processing and offline analysis.
pub trait WDFCircuit<T: WDFloat> {
    /// Processes a single input sample and returns the output sample.
    fn process(&mut self, sample: T) -> T;

    fn circuit_name(&self) -> String {
        let full_type = std::any::type_name::<Self>();
        full_type
            .split('<')
            .next()
            .unwrap_or(full_type)
            .split("::")
            .last()
            .unwrap_or(full_type)
            .into()
    }

    /// Benchmarks the circuit's processing speed.
    fn bench(&mut self, n_seconds: T, fs: T)
    where
        Self: Sized,
    {
        let row = bench_row_named(self.circuit_name(), self, n_seconds, fs);
        println!(
            "Results of processing {} seconds of a random signal at {} khz:",
            n_seconds,
            fs / num_t(1e3)
        );
        print_bench_table(&[row]);
    }

    /// Processes a block of samples.
    fn process_signal(&mut self, signal: &[T]) -> Vec<T> {
        signal.iter().map(|x| self.process(*x)).collect()
    }

    fn process_signal_in_place(&mut self, signal: &mut [T]) {
        for sample in signal.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    /// Calculates the impulse response of the circuit.
    fn get_impulse_response(&mut self, n_samples: usize) -> Vec<T> {
        self.reset();

        let mut impulse = vec![T::zero(); n_samples];
        impulse[0] = T::one();

        self.process_signal(&impulse)
    }

    /// Calculates the complex frequency response using an FFT of the impulse response.
    #[cfg(feature = "fft")]
    fn get_frequency_response(&mut self, n_samples: usize) -> Vec<Complex<T>> {
        let mut ir = self.get_impulse_response(n_samples);
        let mut planner = RealFftPlanner::<T>::new();
        let fft = planner.plan_fft_forward(n_samples);
        let mut output = fft.make_output_vec();
        fft.process(&mut ir, &mut output)
            .expect("Error computing fft");
        output
    }

    /// Calculates the magnitude response (in dB) of the circuit.
    #[cfg(feature = "fft")]
    fn get_magnitude_response(&mut self, n_samples: usize) -> Vec<T> {
        let freq_response = self.get_frequency_response(n_samples);
        freq_response
            .iter()
            .map(|c| gain_to_db((c.re * c.re + c.im * c.im).sqrt()))
            .collect()
    }

    /// Calculates the phase response of the circuit.
    #[cfg(feature = "fft")]
    fn get_phase_response(&mut self, n_samples: usize) -> Vec<T> {
        let freq_response = self.get_frequency_response(n_samples);
        freq_response.iter().map(|c| c.im.atan2(c.re)).collect()
    }

    #[cfg(feature = "plot")]
    fn plot_magnitude_response(&mut self, n_samples: usize, fs: T) {
        self.reset();
        let mag_response: Vec<f64> = self
            .get_magnitude_response(n_samples)
            .iter()
            .map(|x| x.to_f64().unwrap())
            .collect();
        self.reset();

        let title = format!("{} — Magnitude Response", self.circuit_name());
        plot_magnitude_response(&mag_response, fs.to_f64().unwrap(), &title);
    }

    #[cfg(feature = "plot")]
    fn plot_magnitude_response_list<F>(
        &mut self,
        n_samples: usize,
        fs: T,
        setter: F,
        values: Vec<T>,
    ) where
        F: Fn(&mut Self, T),
        Self: Sized,
    {
        let labels: Vec<String> = values.iter().map(|v| v.to_string()).collect();
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();

        let mag_responses: Vec<Vec<f64>> = values
            .iter()
            .map(|val| {
                self.reset();
                setter(self, *val);
                self.get_magnitude_response(n_samples)
                    .iter()
                    .map(|x| x.to_f64().unwrap())
                    .collect()
            })
            .collect();

        self.reset();

        let setter_name = std::any::type_name::<F>()
            .split("::")
            .last()
            .unwrap_or("param");
        let title = format!(
            "{} — Magnitude Response ({})",
            self.circuit_name(),
            setter_name
        );
        let response_refs: Vec<&[f64]> = mag_responses.iter().map(|v| v.as_slice()).collect();
        plot_magnitude_responses_labels(&response_refs, &label_refs, fs.to_f64().unwrap(), &title);
    }

    #[cfg(feature = "plot")]
    fn plot_impulse_response(&mut self, n_samples: usize) {
        self.reset();
        let impulse_response: Vec<f64> = self
            .get_impulse_response(n_samples)
            .iter()
            .map(|x| x.to_f64().unwrap())
            .collect();
        self.reset();

        let title = format!("{} — Impulse Response", self.circuit_name());
        plot_impulse_response_signal(&impulse_response, &title);
    }

    #[cfg(feature = "plot")]
    fn plot_transfer_curve(&mut self, input_range: (T, T), n_points: usize, settle_samples: usize) {
        self.reset();
        let (min_in, max_in) = input_range;
        let step = (max_in - min_in) / num_t(n_points as f64);

        let mut xs: Vec<f64> = Vec::with_capacity(n_points);
        let mut ys: Vec<f64> = Vec::with_capacity(n_points);

        let mut x = min_in;
        for _ in 0..n_points {
            for _ in 0..settle_samples {
                self.process(x);
            }
            let y = self.process(x);
            xs.push(x.to_f64().unwrap());
            ys.push(y.to_f64().unwrap());
            x = x + step;
        }

        let title = format!("{} — Transfer Curve", self.circuit_name());
        let layout = Layout::new()
            .title(Title::with_text(title))
            .x_axis(Axis::new().title(Title::with_text("Input")))
            .y_axis(Axis::new().title(Title::with_text("Output")));

        let mut plot = Plot::new();
        let trace = Scatter::new(xs, ys).name("transfer curve");
        plot.add_trace(trace);
        plot.set_layout(layout);
        plot.show();
        self.reset();
    }

    /// Implementation provided by derive(WDFCircuit) macro. Calls `reset` on every component in the circuit tree
    fn reset(&mut self) {}
}
