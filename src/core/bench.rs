//! Benchmarking utilities. See [`compare_circuits`] and [`WDFCircuit::bench`]

use crate::core::{circuit::WDFCircuit, utils::*, wdf::*};

#[derive(Clone, Debug)]
pub struct BenchMetrics {
    pub elapsed_ms: f64,
    pub ratio: f64,
    pub ns_per_sample: f64,
}

#[derive(Clone, Debug)]
pub struct BenchRow {
    pub circuit: String,
    pub metrics: BenchMetrics,
}

pub fn bench_metrics_for<T: WDFloat, C: WDFCircuit<T> + ?Sized>(
    circuit: &mut C,
    n_seconds: T,
    fs: T,
) -> BenchMetrics {
    let n_samples = T::to_usize(&(n_seconds * fs)).unwrap();

    let mut buffer = rand_wave(n_samples);

    let now = std::time::Instant::now();

    circuit.process_signal_in_place(std::hint::black_box(&mut buffer));
    std::hint::black_box(&buffer);

    let elapsed = now.elapsed();

    let elapsed_seconds = elapsed.as_secs_f64();
    let ns_per_sample = if n_samples > 0 {
        (elapsed_seconds * 1e9) / n_samples as f64
    } else {
        0.0
    };

    BenchMetrics {
        elapsed_ms: elapsed_seconds * 1e3,
        ratio: if elapsed_seconds > 0.0 {
            n_seconds.to_f64().unwrap_or(0.0) / elapsed_seconds
        } else {
            0.0
        },
        ns_per_sample,
    }
}

pub fn bench_row<T: WDFloat, C: WDFCircuit<T>>(circuit: &mut C, n_seconds: T, fs: T) -> BenchRow {
    BenchRow {
        circuit: type_of::<C>(),
        metrics: bench_metrics_for(circuit, n_seconds, fs),
    }
}

pub fn bench_row_named<T: WDFloat, C: WDFCircuit<T>>(
    name: impl Into<String>,
    circuit: &mut C,
    n_seconds: T,
    fs: T,
) -> BenchRow {
    BenchRow {
        circuit: name.into(),
        metrics: bench_metrics_for(circuit, n_seconds, fs),
    }
}

pub fn compare_circuits_rows<T: WDFloat>(
    mut circuits: Vec<&mut dyn WDFCircuit<T>>,
    n_seconds: T,
    fs: T,
) -> Vec<BenchRow> {
    circuits
        .iter_mut()
        .map(|circuit| BenchRow {
            circuit: circuit.circuit_name(),
            metrics: bench_metrics_for(*circuit, n_seconds, fs),
        })
        .collect()
}

pub fn compare_circuits<T: WDFloat>(circuits: Vec<&mut dyn WDFCircuit<T>>, n_seconds: T, fs: T) {
    let rows = compare_circuits_rows(circuits, n_seconds, fs);
    println!(
        "Comparison of processing {} seconds of a random signal at {} khz:",
        n_seconds,
        fs / num_t(1e3)
    );
    print_bench_table(&rows);
}

pub fn print_bench_table(rows: &[BenchRow]) {
    let header = ["Circuit", "Time (ms)", "x real-time", "ns/sample"];
    let mut body: Vec<[String; 4]> = rows
        .iter()
        .map(|row| {
            [
                row.circuit.clone(),
                format!("{:.2}", row.metrics.elapsed_ms),
                format!("{:.2}", row.metrics.ratio),
                format!("{:.2}", row.metrics.ns_per_sample),
            ]
        })
        .collect();

    let mut widths = [
        header[0].len(),
        header[1].len(),
        header[2].len(),
        header[3].len(),
    ];
    for row in &body {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }

    let separator = format!(
        "├─{}─┼─{}─┼─{}─┼─{}─┤",
        "─".repeat(widths[0]),
        "─".repeat(widths[1]),
        "─".repeat(widths[2]),
        "─".repeat(widths[3])
    );

    println!(
        "\n┌─{}─┬─{}─┬─{}─┬─{}─┐",
        "─".repeat(widths[0]),
        "─".repeat(widths[1]),
        "─".repeat(widths[2]),
        "─".repeat(widths[3])
    );
    println!(
        "│ {:<w0$} │ {:>w1$} │ {:>w2$} │ {:>w3$} │",
        header[0],
        header[1],
        header[2],
        header[3],
        w0 = widths[0],
        w1 = widths[1],
        w2 = widths[2],
        w3 = widths[3]
    );
    println!("{}", separator);
    for row in body.drain(..) {
        println!(
            "│ {:<w0$} │ {:>w1$} │ {:>w2$} │ {:>w3$} │",
            row[0],
            row[1],
            row[2],
            row[3],
            w0 = widths[0],
            w1 = widths[1],
            w2 = widths[2],
            w3 = widths[3]
        );
    }
    println!(
        "└─{}─┴─{}─┴─{}─┴─{}─┘\n",
        "─".repeat(widths[0]),
        "─".repeat(widths[1]),
        "─".repeat(widths[2]),
        "─".repeat(widths[3])
    );
}
