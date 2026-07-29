//! Root components of a Wave Digital Filter tree.
//!
//! The root component is the entry point of the WDF tree and is responsible for
//! initiating the wave propagation cycle.

use super::traits::*;
use crate::core::utils::*;
use core::any::Any;
use core::cell::UnsafeCell;
use core::ptr::NonNull;

#[derive(Default)]
struct IVSInternal<T: WDFloat> {
    params: WDFParams<T>,
    voltage: T,
}

/// An Ideal Voltage Source used as the root of a WDF tree.
///
/// Unlike a `ResistiveVoltageSource`, an `IdealVoltageSource` has zero
/// internal resistance and must be placed at the root of the tree
pub struct IdealVoltageSource<T, N>
where
    T: WDFloat,
    N: WDF<T>,
{
    /// The component connected to the source.
    pub next: N,
    inner: UnsafeCell<IVSInternal<T>>,
}

impl<T, N> IdealVoltageSource<T, N>
where
    T: WDFloat,
    N: WDF<T>,
{
    /// Creates a new `IdealVoltageSource` connected to `next`.
    pub fn new(next: N) -> Self {
        let vs = Self {
            inner: Default::default(),
            next,
        };
        vs.calc_impedance();
        vs
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut IVSInternal<T> {
        unsafe { &mut *self.inner.get() }
    }
}

impl<T, N> RootWDF<T> for IdealVoltageSource<T, N>
where
    T: WDFloat,
    N: WDF<T>,
{
    type Next = N;

    #[inline(always)]
    fn next(&self) -> &Self::Next {
        &self.next
    }
}

impl<T, N> WDF<T> for IdealVoltageSource<T, N>
where
    T: WDFloat,
    N: WDF<T>,
{
    #[inline(always)]
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.inner.get()).params }
    }

    /// An ideal source adopts the impedance of the connected subtree.
    fn calc_impedance(&self) {
        self.next.calc_impedance();
        let next_rp = self.next.params().rp;
        let next_g = self.next.params().g;

        let i = self.inner_mut();
        i.params.rp = next_rp;
        i.params.g = next_g;
    }

    /// Retune the subtree first, then re-adopt: the impedance this root takes on
    /// is only meaningful once its children hold the new rate.
    fn set_sample_rate(&self, fs: T) {
        self.next.set_sample_rate(fs);
        self.calc_impedance();
    }

    /// For an ideal source, $b = 2 \cdot V - a$.
    fn get_reflected_wave(&self) -> T {
        let i = self.inner_mut();
        i.params.b = num_t::<T>(2.0) * i.voltage - i.params.a;
        i.params.b
    }

    fn set_incident_wave(&self, a: T) {
        self.inner_mut().params.a = a;
    }

    fn find_by_id_ptr(&self, id: &str) -> Option<NonNull<dyn Any>> {
        if self.params().id == Some(id) {
            let raw_ptr = self as *const Self as *mut Self as *mut dyn Any;
            return NonNull::new(raw_ptr);
        }
        self.next.find_by_id_ptr(id)
    }

    fn recurse_print(&self, prefix: &str) {
        self.next.print_tree(prefix, true);
    }

    #[inline(always)]
    fn set_voltage(&self, v_in: T) {
        self.inner_mut().voltage = v_in;
    }
}

//====================================================================================

#[derive(Default)]
struct ICSInternal<T: WDFloat> {
    params: WDFParams<T>,
    i_s: T,
    two_r: T,
    two_r_i_s: T,
}

/// An Ideal Current Source used as the root of a WDF tree.
pub struct IdealCurrentSource<T: WDFloat, N: WDF<T>> {
    /// The component connected to the source.
    pub next: N,
    inner: UnsafeCell<ICSInternal<T>>,
}

impl<T: WDFloat, N: WDF<T>> IdealCurrentSource<T, N> {
    pub fn new(next: N) -> Self {
        let ics = Self {
            inner: Default::default(),
            next,
        };
        ics.calc_impedance();
        ics
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut ICSInternal<T> {
        unsafe { &mut *self.inner.get() }
    }
}

impl<T: WDFloat, N: WDF<T>> WDF<T> for IdealCurrentSource<T, N> {
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.inner.get()).params }
    }

    fn calc_impedance(&self) {
        let i = self.inner_mut();
        i.two_r = num_t::<T>(2.0) * self.next.params().rp;
        i.two_r_i_s = i.two_r * i.i_s;
    }

    fn get_reflected_wave(&self) -> T {
        let i = self.inner_mut();
        i.params.b = i.two_r_i_s + i.params.a;
        i.params.b
    }

    fn set_incident_wave(&self, a: T) {
        self.inner_mut().params.a = a
    }

    fn find_by_id_ptr(&self, id: &str) -> Option<NonNull<dyn Any>> {
        if self.params().id == Some(id) {
            let raw_ptr = self as *const Self as *mut Self as *mut dyn Any;
            return NonNull::new(raw_ptr);
        }
        self.next.find_by_id_ptr(id)
    }

    fn recurse_print(&self, prefix: &str) {
        self.next.print_tree(prefix, true);
    }

    fn set_sample_rate(&self, fs: T) {
        self.next.set_sample_rate(fs);
        self.calc_impedance();
    }

    fn set_current(&self, current: T) {
        let i = self.inner_mut();
        i.i_s = current;
        i.two_r_i_s = i.two_r * i.i_s;
    }
}

impl<T: WDFloat, N: WDF<T>> RootWDF<T> for IdealCurrentSource<T, N> {
    type Next = N;

    fn next(&self) -> &Self::Next {
        &self.next
    }
}

//====================================================================================

struct SwitchInternal<T: WDFloat> {
    params: WDFParams<T>,
    closed: bool,
}

/// A simple electronic Switch used as the root of a WDF tree.
///
/// The switch can be either open or closed, effectively changing the
/// boundary condition of the WDF tree.
pub struct Switch<T: WDFloat, N: WDF<T>> {
    /// The component connected to the switch.
    pub next: N,
    inner: UnsafeCell<SwitchInternal<T>>,
}

impl<T: WDFloat, N: WDF<T>> RootWDF<T> for Switch<T, N> {
    type Next = N;

    #[inline(always)]
    fn next(&self) -> &Self::Next {
        &self.next
    }
}

impl<T: WDFloat, N: WDF<T>> Switch<T, N> {
    /// Creates a new `Switch` in the specified state.
    pub fn new(next: N, closed: bool) -> Self {
        let sw = Self {
            next,
            inner: UnsafeCell::new(SwitchInternal {
                params: WDFParams::default(),
                closed,
            }),
        };
        sw.calc_impedance();
        sw
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut SwitchInternal<T> {
        unsafe { &mut *self.inner.get() }
    }

    /// Changes the switch state and triggers an impedance recalculation.
    pub fn set_closed(&self, closed: bool) {
        self.inner_mut().closed = closed;
        self.calc_impedance();
    }
}

impl<T: WDFloat, N: WDF<T>> WDF<T> for Switch<T, N> {
    #[inline(always)]
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.inner.get()).params }
    }

    fn calc_impedance(&self) {
        self.next.calc_impedance();
        let next_p = self.next.params();
        let i = self.inner_mut();
        i.params.rp = next_p.rp;
        i.params.g = next_p.g;
    }

    /// Reflects waves back into the connected subtree based on switch state.
    fn get_reflected_wave(&self) -> T {
        let b_next = self.next.get_reflected_wave();
        let i = self.inner_mut();

        i.params.b = if i.closed { -b_next } else { b_next };
        i.params.b
    }

    fn set_incident_wave(&self, a: T) {
        let i = self.inner_mut();
        i.params.a = a;
        self.next.set_incident_wave(if i.closed { -a } else { a });
    }

    fn find_by_id_ptr(&self, id: &str) -> Option<NonNull<dyn Any>> {
        if self.params().id == Some(id) {
            let raw_ptr = self as *const Self as *mut Self as *mut dyn Any;
            return NonNull::new(raw_ptr);
        }
        self.next.find_by_id_ptr(id)
    }

    fn recurse_print(&self, prefix: &str) {
        self.next.print_tree(prefix, true);
    }

    fn set_sample_rate(&self, fs: T) {
        self.next.set_sample_rate(fs);
        self.calc_impedance();
    }
}
