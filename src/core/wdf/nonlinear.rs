//! Non-linear Wave Digital Filter components.
//!
//! This module implements non-linear elements like diodes using iterative or
//! approximated solvers (e.g., Lambert W function) to resolve the wave-domain
//! equations.

use super::traits::*;
use crate::core::utils::*;
use core::any::Any;
use core::cell::UnsafeCell;
use core::ptr::NonNull;

#[derive(Default)]
struct DiodeInternal<T: WDFloat> {
    params: WDFParams<T>,
    /// Reverse saturation current ($I_S$) of the diode.
    i_s: T,
    /// Thermal voltage ($V_T$) of the diode, typically around 25.85 mV at room temperature.
    v_t: T,
    /// Precomputed constants for the Lambert W approximation to speed up calculations.
    one_over_vt: T,
    two_r_is: T,
    r_is_over_vt: T,
    log_r_is_over_vt: T,
}

/// A single Diode model.
///
/// The diode reflection is calculated using a fast approximation of the
/// Lambert W function, which allows for an explicit solution to the
/// transendental diode equation in the wave domain.
pub struct Diode<T: WDFloat, N: WDF<T>> {
    /// The component connected to the diode.
    pub next: N,
    inner: UnsafeCell<DiodeInternal<T>>,
}

impl<T: WDFloat, N: WDF<T>> RootWDF<T> for Diode<T, N> {
    type Next = N;

    #[inline(always)]
    fn next(&self) -> &Self::Next {
        &self.next
    }
}

impl<T: WDFloat, N: WDF<T>> Diode<T, N> {
    /// Creates a new `Diode` connected to `next`.
    ///
    /// `i_s` is the reverse saturation current, and `n_diodes` allows modeling
    /// multiple diodes in series.
    pub fn new(next: N, i_s: T, n_diodes: T) -> Self {
        let v_t_base = num_t::<T>(25.8e-3);
        let v_t = v_t_base * n_diodes;

        let d = Self {
            next,
            inner: DiodeInternal {
                params: Default::default(),
                i_s,
                v_t,
                one_over_vt: T::one() / v_t,
                ..Default::default()
            }
            .into(),
        };
        d.calc_impedance();
        d
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut DiodeInternal<T> {
        unsafe { &mut *self.inner.get() }
    }

    /// Updates the diode's physical parameters.
    pub fn set_diode_params(&self, i_s: T, n_diodes: T) {
        let i = self.inner_mut();
        i.i_s = i_s;
        i.v_t = n_diodes * num_t::<T>(25.8e-3);
        i.one_over_vt = T::one() / i.v_t;
        self.calc_impedance();
    }
}

impl<T: WDFloat, N: WDF<T>> WDF<T> for Diode<T, N> {
    #[inline(always)]
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.inner.get()).params }
    }

    fn calc_impedance(&self) {
        self.next.calc_impedance();
        let next_rp = self.next.params().rp;
        let i = self.inner_mut();

        i.two_r_is = num_t::<T>(2.0) * next_rp * i.i_s;
        i.r_is_over_vt = next_rp * i.i_s * i.one_over_vt;
        i.log_r_is_over_vt = i.r_is_over_vt.ln();

        i.params.rp = next_rp;
        i.params.g = T::one() / next_rp;
    }

    /// Calculates reflection using the Lambert W approximation.
    fn get_reflected_wave(&self) -> T {
        let next_b = self.next.get_reflected_wave();
        let i = self.inner_mut();
        i.params.a = next_b;

        let offset = i.log_r_is_over_vt + i.params.a * i.one_over_vt + i.r_is_over_vt;
        i.params.b = i.params.a + i.two_r_is - (num_t::<T>(2.0) * i.v_t) * omega_approx(offset);

        i.params.b
    }

    fn set_incident_wave(&self, a: T) {
        self.inner_mut().params.a = a;
        self.next.set_incident_wave(a);
    }

    fn recurse_print(&self, prefix: &str) {
        self.next.print_tree(prefix, true);
    }

    fn for_each_child(&self, f: &mut dyn FnMut(&dyn WDF<T>)) {
        f(&self.next);
    }

    fn set_sample_rate(&self, fs: T) {
        self.for_each_child(&mut |child| {
            child.set_sample_rate(fs);
        });
        self.calc_impedance();
    }
}

//====================================================================================

#[derive(Default)]
struct DiodePairInternal<T: WDFloat> {
    params: WDFParams<T>,
    is: T,
    vt: T,
    one_over_vt: T,
    two_r_is: T,
    r_is_over_vt: T,
    log_r_is_over_vt: T,
}

/// An anti-parallel Diode Pair.
///
/// Models two diodes connected in opposite directions, commonly used in
/// distortion and clipping circuits.
pub struct DiodePair<T: WDFloat, N: WDF<T>> {
    /// The component connected to the diode pair.
    pub next: N,
    inner: UnsafeCell<DiodePairInternal<T>>,
}

impl<T: WDFloat, N: WDF<T>> RootWDF<T> for DiodePair<T, N> {
    type Next = N;

    #[inline(always)]
    fn next(&self) -> &Self::Next {
        &self.next
    }
}

impl<T: WDFloat, N: WDF<T>> DiodePair<T, N> {
    /// Creates a new `DiodePair` connected to `next`.
    pub fn new(next: N, is: T) -> Self {
        let vt = num_t::<T>(25.85e-3 * 2.0);

        let dp = Self {
            next,
            inner: DiodePairInternal {
                is,
                vt,
                one_over_vt: T::one() / vt,
                ..Default::default()
            }
            .into(),
        };
        dp.calc_impedance();
        dp
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut DiodePairInternal<T> {
        unsafe { &mut *self.inner.get() }
    }

    /// Updates the physical parameters for the diode pair.
    pub fn set_diode_params(&self, is: T, vt: T) {
        let i = self.inner_mut();
        i.is = is;
        i.vt = num_t::<T>(2.0) * vt;
        i.one_over_vt = T::one() / i.vt;
        self.calc_impedance();
    }
}

impl<T: WDFloat, N: WDF<T>> WDF<T> for DiodePair<T, N> {
    #[inline(always)]
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.inner.get()).params }
    }

    fn calc_impedance(&self) {
        self.next.calc_impedance();
        let next_rp = self.next.params().rp;
        let i = self.inner_mut();

        i.two_r_is = num_t::<T>(2.0) * next_rp * i.is;
        i.r_is_over_vt = next_rp * i.is * i.one_over_vt;
        i.log_r_is_over_vt = i.r_is_over_vt.ln();

        let p = self.params_mut();
        p.rp = next_rp;
        p.g = T::one() / next_rp;
    }

    /// Calculates reflection for the anti-parallel pair using the Lambert W approximation.
    fn get_reflected_wave(&self) -> T {
        let next_b = self.next.get_reflected_wave();
        let i = self.inner_mut();
        i.params.a = next_b;

        let lam = i.params.a.signum();
        let lam_a_over_vt = lam * i.params.a * i.one_over_vt;

        i.params.b = i.params.a
            - (num_t::<T>(2.0) * i.vt)
                * lam
                * (omega_approx(i.log_r_is_over_vt + lam_a_over_vt)
                    - omega_approx(i.log_r_is_over_vt - lam_a_over_vt));

        i.params.b
    }

    fn set_incident_wave(&self, a: T) {
        self.inner_mut().params.a = a;
    }

    fn find_by_id_ptr(&self, id: &str) -> Option<NonNull<dyn Any>> {
        if self.params().id == Some(id) {
            return NonNull::new(self as *const Self as *mut Self as *mut dyn Any);
        }

        self.next.find_by_id_ptr(id)
    }

    fn recurse_print(&self, prefix: &str) {
        self.next.print_tree(prefix, true);
    }

    fn for_each_child(&self, f: &mut dyn FnMut(&dyn WDF<T>)) {
        f(&self.next);
    }

    fn set_sample_rate(&self, fs: T) {
        self.for_each_child(&mut |child| {
            child.set_sample_rate(fs);
        });
        self.calc_impedance();
    }
}
