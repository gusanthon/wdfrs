//! Composite leaf components that bundle common RC topologies.
//!
//! These components provide single-port equivalents of common resistor-capacitor
//! networks. Compared to building the equivalent two-component tree, they avoid
//! the overhead of a separate adaptor node and store only the necessary state.

use crate::core::{utils::num_t, wdf::*};
use core::cell::UnsafeCell;

#[derive(Default)]
struct ResCapParallelInternal<T: WDFloat> {
    params: WDFParams<T>,
    resistance: T,
    capacitance: T,
    two_rc_over_two_rc_plus_t: T,
    dt: T,
    z: T,
}

/// One-port equivalent of a resistor in parallel with a capacitor.
///
/// The model is discretized with a trapezoidal/bilinear transform and keeps
/// one internal state sample (`z`) to represent reactive memory.
pub struct ResistorCapacitorParallel<T: WDFloat>(UnsafeCell<ResCapParallelInternal<T>>);

impl<T: WDFloat> ResistorCapacitorParallel<T> {
    /// Creates a parallel RC one-port.
    ///
    /// - `resistance`: resistance in ohms.
    /// - `capacitance`: capacitance in farads.
    /// - `fs`: sample rate in hertz.
    pub fn new(resistance: T, capacitance: T, fs: T) -> Self {
        let params = WDFParams {
            fs: Some(fs),
            ..Default::default()
        };

        let inner = ResCapParallelInternal {
            params,
            resistance,
            capacitance,
            dt: T::one() / fs,
            ..Default::default()
        }
        .into();

        let rcp = Self(inner);

        rcp.calc_impedance();

        rcp
    }

    pub fn set_capacitance(&self, capacitance: T) {
        self.inner_mut().capacitance = capacitance;
        self.calc_impedance();
    }

    pub fn set_resistance(&self, resistance: T) {
        self.inner_mut().resistance = resistance;
        self.calc_impedance();
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut ResCapParallelInternal<T> {
        unsafe { &mut *self.0.get() }
    }
}

impl<T: WDFloat> WDF<T> for ResistorCapacitorParallel<T> {
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.0.get()).params }
    }

    fn calc_impedance(&self) {
        let i = self.inner_mut();
        let p = self.params_mut();
        let two_r_c = num_t::<T>(2.0) * i.capacitance * i.resistance;
        p.rp = i.resistance * i.dt / (two_r_c + i.dt);
        p.g = T::one() / p.rp;
        i.two_rc_over_two_rc_plus_t = two_r_c / (two_r_c + i.dt);
    }

    fn get_reflected_wave(&self) -> T {
        let i = self.inner_mut();
        i.params.b = i.two_rc_over_two_rc_plus_t * i.z;
        i.params.b
    }

    fn set_incident_wave(&self, a: T) {
        let i = self.inner_mut();
        let p = self.params_mut();

        p.a = a;
        i.z = p.b + p.a - i.z;
    }

    fn set_sample_rate(&self, fs: T) {
        let p = self.params_mut();
        let i = self.inner_mut();

        p.fs = Some(fs);
        i.dt = T::one() / fs;

        self.calc_impedance();
    }

    fn reset(&self) {
        let i = self.inner_mut();
        i.params.reset();
        i.z = T::zero();
    }
}

//====================================================================================

#[derive(Default)]
struct ResCapSeriesInternal<T: WDFloat> {
    params: WDFParams<T>,
    res: T,
    cap: T,
    t_over_t_plus_2rc: T,
    dt: T,
    z: T,
}

/// One-port equivalent of a resistor in series with a capacitor.
///
/// Like [`ResistorCapacitorParallel`], this element stores one internal
/// state sample and exposes standard WDF wave behavior.
pub struct ResistorCapacitorSeries<T: WDFloat>(UnsafeCell<ResCapSeriesInternal<T>>);

impl<T: WDFloat> ResistorCapacitorSeries<T> {
    /// Creates a series RC one-port.
    ///
    /// - `resistance`: resistance in ohms.
    /// - `capacitance`: capacitance in farads.
    /// - `fs`: sample rate in hertz.
    pub fn new(resistance: T, capacitance: T, fs: T) -> Self {
        let params = WDFParams {
            fs: Some(fs),
            ..Default::default()
        };

        let inner = ResCapSeriesInternal {
            params,
            res: resistance,
            cap: capacitance,
            dt: T::one() / fs,
            ..Default::default()
        }
        .into();

        let rcs = Self(inner);

        rcs.calc_impedance();

        rcs
    }

    pub fn set_capacitance(&self, capacitance: T) {
        self.inner_mut().cap = capacitance;
        self.calc_impedance();
    }

    pub fn set_resistance(&self, resistance: T) {
        self.inner_mut().res = resistance;
        self.calc_impedance();
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut ResCapSeriesInternal<T> {
        unsafe { &mut *self.0.get() }
    }
}

impl<T: WDFloat> WDF<T> for ResistorCapacitorSeries<T> {
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.0.get()).params }
    }

    fn calc_impedance(&self) {
        let i = self.inner_mut();
        let p = self.params_mut();

        p.rp = i.dt / (num_t::<T>(2.0) * i.cap) + i.res;
        p.g = T::one() / p.rp;

        i.t_over_t_plus_2rc = i.dt / (num_t::<T>(2.0) * i.cap * i.res + i.dt);
    }

    fn get_reflected_wave(&self) -> T {
        let z_val = self.inner_mut().z;
        let p = self.params_mut();
        p.b = -z_val;
        p.b
    }
    fn set_incident_wave(&self, a: T) {
        let i = self.inner_mut();
        let p = self.params_mut();

        p.a = a;
        i.z = i.z - i.t_over_t_plus_2rc * (a + i.z);
    }

    fn set_sample_rate(&self, fs: T) {
        let p = self.params_mut();
        let i = self.inner_mut();

        p.fs = Some(fs);
        i.dt = T::one() / fs;

        self.calc_impedance();
    }

    fn reset(&self) {
        let i = self.inner_mut();
        i.params.reset();
        i.z = T::zero();
    }
}
