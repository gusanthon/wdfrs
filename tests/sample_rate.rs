//! A sample-rate change has to reach the root, not just the subtree under it.
//!
//! Reactive components hold `fs` and derive their impedance from it, so
//! `set_tree_sample_rate` moving every capacitor is only half the job: a root
//! that adopts its subtree's impedance has to re-adopt afterwards. Getting the
//! order wrong is invisible in the wave values and shows up only as a root
//! `rp` that disagrees with the child it is supposed to match.

use wdfrs::core::utils::num_t;
use wdfrs::core::wdf::*;

fn tree<T: WDFloat>(fs: T) -> IdealVoltageSource<T, SeriesAdaptor<T, Resistor<T>, Capacitor<T>>> {
    IdealVoltageSource::new(SeriesAdaptor::new(
        Resistor::new(num_t(2.2e3)),
        Capacitor::new(num_t(100e-9), fs),
    ))
}

#[test]
fn root_impedance_follows_the_tree_sample_rate() {
    let root = tree(48e3_f64);
    let rp_at_48k = root.params().rp;

    root.set_tree_sample_rate(96e3);
    let rp_at_96k = root.params().rp;

    // The capacitor's impedance is 1/(2·C·fs), so doubling fs has to move it.
    assert_ne!(
        rp_at_48k, rp_at_96k,
        "root rp did not change when the sample rate did"
    );

    // An ideal source adopts the subtree impedance; after the change it must
    // agree with the subtree as re-derived at the *new* rate.
    assert_eq!(
        rp_at_96k,
        root.next.params().rp,
        "root rp is stale relative to its own subtree"
    );
}

#[test]
fn set_tree_sample_rate_matches_building_at_that_rate() {
    let retuned = tree(48e3_f64);
    retuned.set_tree_sample_rate(96e3);

    let built = tree(96e3_f64);

    assert_eq!(retuned.params().rp, built.params().rp);
    assert_eq!(retuned.next.params().rp, built.next.params().rp);
}
