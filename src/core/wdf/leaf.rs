//! Leaf components of a Wave Digital Filter tree.
//!
//! This module contains basic electronic components like resistors, capacitors,
//! and inductors. These components serve as the leaves of the WDF tree,
//! where wave reflections begin.

use super::traits::*;
use crate::core::utils::*;
use core::cell::UnsafeCell;

/// A standard electronic Resistor.
///
/// In the wave domain, a resistor is a memoryless component with a constant
/// reflected wave of zero (assuming no noise or internal source).
pub struct Resistor<T: WDFloat>(UnsafeCell<WDFParams<T>>);

impl<T: WDFloat> Resistor<T> {
    /// Creates a new `Resistor` with the given `resistance`.
    pub fn new(resistance: T) -> Self {
        let r = Resistor(
            WDFParams {
                rp: resistance,
                g: T::one() / resistance,
                ..Default::default()
            }
            .into(),
        );
        r.calc_impedance();
        r
    }

    /// Sets a new resistance value and updates the port impedance.
    pub fn set_resistance(&self, resistance: T) {
        let p = self.params_mut();
        p.rp = resistance;
        self.calc_impedance();
    }
}

impl<T: WDFloat> WDF<T> for Resistor<T> {
    fn params_ptr(&self) -> *mut WDFParams<T> {
        self.0.get()
    }

    fn calc_impedance(&self) {
        let p = self.params_mut();
        p.g = T::one() / p.rp;
    }

    /// For a resistor, $b = 0$ (no internal source, no reactive memory).
    fn get_reflected_wave(&self) -> T {
        let p = self.params_mut();
        p.b = T::zero();
        p.b
    }

    fn set_incident_wave(&self, a: T) {
        self.params_mut().a = a;
    }
}

//====================================================================================

struct CapInternal<T: WDFloat> {
    params: WDFParams<T>,
    capacitance: T,
    z: T,
}

/// A Capacitor, discretized using the Bilinear Transform.
///
/// The capacitor maintains an internal state (`z`) representing the delayed
/// incident wave.
pub struct Capacitor<T: WDFloat>(UnsafeCell<CapInternal<T>>);

impl<T: WDFloat> Capacitor<T> {
    /// Creates a new `Capacitor` with the given `capacitance` and sampling frequency `fs`.
    pub fn new(capacitance: T, fs: T) -> Self {
        let cap = Capacitor(
            CapInternal {
                params: WDFParams {
                    fs: Some(fs),
                    ..Default::default()
                },
                capacitance,
                z: T::zero(),
            }
            .into(),
        );
        cap.calc_impedance();
        cap
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut CapInternal<T> {
        unsafe { &mut *self.0.get() }
    }

    /// Updates the capacitance value.
    pub fn set_capacitance(&self, capacitance: T) {
        let inner = self.inner_mut();
        inner.capacitance = capacitance;
        self.calc_impedance();
    }
}

impl<T: WDFloat> WDF<T> for Capacitor<T> {
    #[inline(always)]
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.0.get()).params }
    }

    /// Calculates the capacitor port impedance: $R_p = 1 / (2 \cdot C \cdot f_s)$.
    fn calc_impedance(&self) {
        let i = self.inner_mut();
        let fs = i.params.fs.unwrap();

        i.params.rp = T::one() / (num_t::<T>(2.0) * i.capacitance * fs);
        i.params.g = T::one() / i.params.rp;
    }

    /// Returns the delayed state as the reflected wave.
    fn get_reflected_wave(&self) -> T {
        let i = self.inner_mut();
        i.params.b = i.z;
        i.params.b
    }

    /// Stores the incident wave into the delayed state `z`.
    fn set_incident_wave(&self, a: T) {
        let i = self.inner_mut();
        i.params.a = a;
        i.z = a;
    }

    fn reset(&self) {
        let i = self.inner_mut();
        i.params.reset();
        i.z = T::zero();
    }
}

//====================================================================================
struct CapAlphaInternal<T: WDFloat> {
    params: WDFParams<T>,
    capacitance: T,
    z: T,
    alpha: T,
    a_coeff: T,
    b_coeff: T,
}

/// A Capacitor, discretized using the Alpha Transform.
///
/// The capacitor maintains an internal state (`z`) representing the delayed
/// incident wave.
pub struct CapacitorAlpha<T: WDFloat>(UnsafeCell<CapAlphaInternal<T>>);

impl<T: WDFloat> CapacitorAlpha<T> {
    pub fn new(capacitance: T, fs: T, alpha: T) -> Self {
        let cap = CapacitorAlpha(
            CapAlphaInternal {
                params: WDFParams {
                    fs: Some(fs),
                    ..Default::default()
                },
                capacitance,
                z: T::zero(),
                alpha,
                a_coeff: (T::one() + alpha) / num_t(2.0),
                b_coeff: (T::one() - alpha) / num_t(2.0),
            }
            .into(),
        );
        cap.calc_impedance();
        cap
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut CapAlphaInternal<T> {
        unsafe { &mut *self.0.get() }
    }

    /// Updates the capacitance value.
    pub fn set_capacitance(&self, capacitance: T) {
        let inner = self.inner_mut();
        inner.capacitance = capacitance;
        self.calc_impedance();
    }
}

impl<T: WDFloat> WDF<T> for CapacitorAlpha<T> {
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.0.get()).params }
    }

    fn calc_impedance(&self) {
        let i = self.inner_mut();
        i.params.rp = T::one() / ((T::one() + i.alpha) * i.capacitance * i.params.fs.unwrap());
        i.params.g = T::one() / i.params.rp;
    }

    fn get_reflected_wave(&self) -> T {
        let i = self.inner_mut();
        i.params.b = i.b_coeff * i.params.b + i.a_coeff * i.z;
        i.params.b
    }

    /// Stores the incident wave into the delayed state `z`.
    fn set_incident_wave(&self, a: T) {
        let i = self.inner_mut();
        i.params.a = a;
        i.z = a;
    }

    fn reset(&self) {
        let i = self.inner_mut();
        i.params.reset();
        i.z = T::zero();
    }
}

//====================================================================================

struct IndInternal<T: WDFloat> {
    params: WDFParams<T>,
    inductance: T,
    z: T,
}

/// An Inductor, discretized using the Bilinear Transform.
///
/// Similar to the capacitor, it maintains internal state `z`, but with a
/// sign inversion in the reflection equation.
pub struct Inductor<T: WDFloat>(UnsafeCell<IndInternal<T>>);

impl<T: WDFloat> Inductor<T> {
    /// Creates a new `Inductor` with the given `inductance` and sampling frequency `fs`.
    pub fn new(inductance: T, fs: T) -> Self {
        let ind = Self(
            IndInternal {
                params: WDFParams {
                    fs: Some(fs),
                    ..Default::default()
                },
                inductance,
                z: T::zero(),
            }
            .into(),
        );
        ind.calc_impedance();
        ind
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut IndInternal<T> {
        unsafe { &mut *self.0.get() }
    }

    /// Updates the inductance value.
    pub fn set_inductance(&self, inductance: T) {
        let i = self.inner_mut();
        i.inductance = inductance;
        self.calc_impedance();
    }
}

impl<T: WDFloat> WDF<T> for Inductor<T> {
    #[inline(always)]
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.0.get()).params }
    }

    /// Calculates the inductor port impedance: $R_p = 2 \cdot L \cdot f_s$.
    fn calc_impedance(&self) {
        let i = self.inner_mut();
        let fs = i.params.fs.unwrap();

        i.params.rp = num_t::<T>(2.0) * i.inductance * fs;
        i.params.g = T::one() / i.params.rp;
    }

    /// For an inductor, $b = -z$.
    fn get_reflected_wave(&self) -> T {
        let i = self.inner_mut();
        i.params.b = -i.z;
        i.params.b
    }

    /// Stores the incident wave into the delayed state `z`.
    fn set_incident_wave(&self, a: T) {
        let i = self.inner_mut();
        i.params.a = a;
        i.z = a;
    }

    fn reset(&self) {
        let i = self.inner_mut();
        i.params.reset();
        i.z = T::zero();
    }
}

//====================================================================================

struct IndAlphaInternal<T: WDFloat> {
    params: WDFParams<T>,
    inductance: T,
    z: T,
    alpha: T,
    a_coeff: T,
    b_coeff: T,
}

/// An Inductor, discretized using the Alpha Transform.
///
/// Similar to the capacitor, it maintains internal state `z`, but with a
/// sign inversion in the reflection equation.
pub struct InductorAlpha<T: WDFloat>(UnsafeCell<IndAlphaInternal<T>>);

impl<T: WDFloat> InductorAlpha<T> {
    /// Creates a new `Inductor` with the given `inductance` and sampling frequency `fs`.
    pub fn new(inductance: T, fs: T, alpha: T) -> Self {
        let ind = Self(
            IndAlphaInternal {
                params: WDFParams {
                    fs: Some(fs),
                    ..Default::default()
                },
                inductance,
                z: T::zero(),
                alpha,
                a_coeff: (T::one() + alpha) / num_t(2.0),
                b_coeff: (T::one() - alpha) / num_t(2.0),
            }
            .into(),
        );
        ind.calc_impedance();
        ind
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut IndAlphaInternal<T> {
        unsafe { &mut *self.0.get() }
    }

    /// Updates the inductance value.
    pub fn set_inductance(&self, inductance: T) {
        let i = self.inner_mut();
        i.inductance = inductance;
        self.calc_impedance();
    }
}

impl<T: WDFloat> WDF<T> for InductorAlpha<T> {
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.0.get()).params }
    }

    fn calc_impedance(&self) {
        let i = self.inner_mut();
        i.params.rp = (T::one() + i.alpha) * i.inductance * i.params.fs.unwrap();
        i.params.g = T::one() / i.params.rp;
    }

    fn get_reflected_wave(&self) -> T {
        let i = self.inner_mut();
        i.params.b = i.b_coeff * i.params.b - i.a_coeff * i.z;
        i.params.b
    }

    /// Stores the incident wave into the delayed state `z`.
    fn set_incident_wave(&self, a: T) {
        let i = self.inner_mut();
        i.params.a = a;
        i.z = a;
    }

    fn reset(&self) {
        let i = self.inner_mut();
        i.params.reset();
        i.z = T::zero();
    }
}

//====================================================================================

struct NonLinIndInternal<T: WDFloat> {
    params: WDFParams<T>,
    z: T,
    inductance: T,
}

/// An [`Inductor`] with a soft-saturation nonlinearity: $z \leftarrow a / \sqrt{a^2 + 1}$.
pub struct NonlinearInductor<T: WDFloat>(UnsafeCell<NonLinIndInternal<T>>);

impl<T: WDFloat> NonlinearInductor<T> {
    pub fn new(inductance: T, fs: T) -> Self {
        let ind = Self(
            NonLinIndInternal {
                params: WDFParams {
                    fs: Some(fs),
                    ..Default::default()
                },
                inductance,
                z: T::zero(),
            }
            .into(),
        );
        ind.calc_impedance();
        ind
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut NonLinIndInternal<T> {
        unsafe { &mut *self.0.get() }
    }

    /// Updates the inductance value.
    pub fn set_inductance(&self, inductance: T) {
        let i = self.inner_mut();
        i.inductance = inductance;
        self.calc_impedance();
    }
}

impl<T: WDFloat> WDF<T> for NonlinearInductor<T> {
    #[inline(always)]
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.0.get()).params }
    }

    /// Calculates the inductor port impedance: $R_p = 2 \cdot L \cdot f_s$.
    fn calc_impedance(&self) {
        let i = self.inner_mut();
        let fs = i.params.fs.unwrap();

        i.params.rp = num_t::<T>(2.0) * i.inductance * fs;
        i.params.g = T::one() / i.params.rp;
    }

    /// For an inductor, $b = -z$.
    fn get_reflected_wave(&self) -> T {
        let i = self.inner_mut();
        i.params.b = -i.z;
        i.params.b
    }

    /// Stores the incident wave into the delayed state `z`.
    fn set_incident_wave(&self, a: T) {
        let i = self.inner_mut();
        i.params.a = a;
        i.z = a / (a * a + T::one()).sqrt();
    }

    fn reset(&self) {
        let i = self.inner_mut();
        i.params.reset();
        i.z = T::zero();
    }
}

//====================================================================================

struct RVSInternal<T: WDFloat> {
    params: WDFParams<T>,
    voltage: T,
    r_val: T,
}

/// A Resistive Voltage Source.
///
/// This represents a voltage source with a series resistance. In the wave
/// domain, it behaves similarly to a resistor but with a constant (per sample)
/// reflected wave equal to the source voltage.
pub struct ResistiveVoltageSource<T: WDFloat>(UnsafeCell<RVSInternal<T>>);

impl<T: WDFloat> ResistiveVoltageSource<T> {
    /// Creates a new `ResistiveVoltageSource`.
    pub fn new(resistance: T) -> Self {
        let rvs = Self(
            RVSInternal {
                params: WDFParams::default(),
                r_val: if resistance.abs() <= T::epsilon() {
                    T::epsilon()
                } else {
                    resistance
                },
                voltage: T::zero(),
            }
            .into(),
        );
        rvs.calc_impedance();
        rvs
    }

    pub fn set_resistance(&self, resistance: T) {
        let i = self.inner_mut();
        i.r_val = if resistance == T::zero() {
            T::epsilon()
        } else {
            resistance
        };
        self.calc_impedance();
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut RVSInternal<T> {
        unsafe { &mut *self.0.get() }
    }
}

impl<T: WDFloat> WDF<T> for ResistiveVoltageSource<T> {
    #[inline(always)]
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.0.get()).params }
    }

    fn calc_impedance(&self) {
        let i = self.inner_mut();
        i.params.rp = i.r_val;
        i.params.g = T::one() / i.params.rp;
    }

    /// For an RVS, the reflected wave is simply the source voltage.
    fn get_reflected_wave(&self) -> T {
        let i = self.inner_mut();
        i.params.b = i.voltage;
        i.params.b
    }

    fn set_incident_wave(&self, a: T) {
        self.inner_mut().params.a = a;
    }

    fn set_voltage(&self, voltage: T) {
        self.inner_mut().voltage = voltage;
    }
}

//====================================================================================

#[derive(Default, Debug)]
struct CapVSInternal<T: WDFloat> {
    cap_val: T,
    v0: T,
    v1: T,
    z: T,
    params: WDFParams<T>,
}

pub struct CapacitiveVoltageSource<T: WDFloat>(UnsafeCell<CapVSInternal<T>>);

impl<T: WDFloat> CapacitiveVoltageSource<T> {
    pub fn new(cap_val: T, fs: T) -> Self {
        let cap_vs = Self(
            CapVSInternal {
                cap_val,
                params: WDFParams {
                    fs: Some(fs),
                    ..Default::default()
                },
                ..Default::default()
            }
            .into(),
        );
        cap_vs.calc_impedance();

        cap_vs
    }

    pub fn set_capacitance_value(&self, new_cap: T) {
        self.inner_mut().cap_val = new_cap;
        self.calc_impedance();
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut CapVSInternal<T> {
        unsafe { &mut *self.0.get() }
    }
}

impl<T: WDFloat> WDF<T> for CapacitiveVoltageSource<T> {
    #[inline(always)]
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.0.get()).params }
    }

    fn calc_impedance(&self) {
        let i = self.inner_mut();
        i.params.rp = T::one() / (num_t::<T>(2.0) * i.cap_val * i.params.fs.unwrap());
        i.params.g = T::one() / i.params.rp;
    }

    fn set_voltage(&self, voltage: T) {
        self.inner_mut().v0 = voltage;
    }

    fn get_reflected_wave(&self) -> T {
        let i = self.inner_mut();
        i.params.b = i.z + i.v0 - i.v1;
        i.v1 = i.v0;
        i.params.b
    }

    fn set_incident_wave(&self, a: T) {
        let i = self.inner_mut();
        i.params.a = a;
        i.z = i.params.a;
    }

    fn reset(&self) {
        let i = self.inner_mut();
        i.z = T::zero();
        i.v0 = T::zero();
        i.v1 = T::zero();
    }
}
