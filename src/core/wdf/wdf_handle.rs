//! Safe handle system for WDF components.
//!
//! This module provides the `WDFHandle` struct and specialized aliases (e.g.,
//! `CapHandle`, `ResHandle`) for safely accessing and modifying WDF components
//! after the circuit tree has been built.

use crate::core::wdf::{
    CapacitiveVoltageSource, Capacitor, Inductor, ParallelAdaptor, ResistiveVoltageSource,
    Resistor, ResistorCapacitorParallel, ResistorCapacitorSeries, SeriesAdaptor, WDFloat, WDF,
};
use core::{marker::PhantomData, ptr::NonNull};

/// A typed pointer to a component inside a built WDF tree, used for runtime parameter changes.
///
/// Obtained automatically via [`RootWDF::get_handle_for_id`](crate::core::wdf::RootWDF::get_handle_for_id):
/// Valid only for the lifetime of the owning circuit root.
pub struct WDFHandle<T: WDFloat, W: WDF<T>> {
    ptr: NonNull<W>,
    _marker: PhantomData<T>,
}

impl<T: WDFloat, W: WDF<T>> WDFHandle<T, W> {
    pub(crate) fn new(ptr: NonNull<W>) -> Self {
        Self {
            ptr,
            _marker: PhantomData,
        }
    }

    /// Creates a placeholder handle before macro-generated initialization patches in the real node.
    ///
    /// # Safety
    /// The returned handle must be overwritten with a valid handle before any method is called on it.
    #[doc(hidden)]
    pub unsafe fn __internal_uninit() -> Self {
        Self::new(NonNull::dangling())
    }

    /// Access the component through a closure.
    fn update<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut W),
    {
        // Safety: The Handle is only created by get_handle_for_id,
        // which guarantees the pointer is valid for the life of the tree.
        unsafe {
            f(self.ptr.as_mut());
        }
    }

    #[inline(always)]
    pub fn wave_to_voltage(&self) -> T {
        unsafe { self.ptr.as_ref().wave_to_voltage() }
    }

    #[inline(always)]
    pub fn wave_to_current(&self) -> T {
        unsafe { self.ptr.as_ref().wave_to_current() }
    }
}

pub type CapHandle<T> = WDFHandle<T, Capacitor<T>>;

impl<T: WDFloat> CapHandle<T> {
    pub fn set_capacitance(&mut self, c: T) {
        self.update(|cap| cap.set_capacitance(c));
    }
}

pub type ResHandle<T> = WDFHandle<T, Resistor<T>>;

impl<T: WDFloat> ResHandle<T> {
    pub fn set_resistance(&mut self, r: T) {
        self.update(|res| res.set_resistance(r));
    }
}

pub type ResCapSerHandle<T> = WDFHandle<T, ResistorCapacitorSeries<T>>;

impl<T: WDFloat> ResCapSerHandle<T> {
    pub fn set_resistance(&mut self, r: T) {
        self.update(|rcs| rcs.set_resistance(r));
    }

    pub fn set_capacitance(&mut self, c: T) {
        self.update(|rcs| rcs.set_capacitance(c));
    }
}

pub type ResCapParHandle<T> = WDFHandle<T, ResistorCapacitorParallel<T>>;

impl<T: WDFloat> ResCapParHandle<T> {
    pub fn set_resistance(&mut self, r: T) {
        self.update(|rcp| rcp.set_resistance(r));
    }

    pub fn set_capacitance(&mut self, c: T) {
        self.update(|rcp| rcp.set_capacitance(c));
    }
}

pub type IndHandle<T> = WDFHandle<T, Inductor<T>>;

impl<T: WDFloat> IndHandle<T> {
    pub fn set_inductance(&mut self, l: T) {
        self.update(|ind| ind.set_inductance(l));
    }
}

pub type ParAdapHandle<T, P1, P2> = WDFHandle<T, ParallelAdaptor<T, P1, P2>>;

pub type SerAdapHandle<T, P1, P2> = WDFHandle<T, SeriesAdaptor<T, P1, P2>>;

pub type ResVSHandle<T> = WDFHandle<T, ResistiveVoltageSource<T>>;

impl<T: WDFloat> ResVSHandle<T> {
    #[inline(always)]
    pub fn set_voltage(&mut self, v: T) {
        self.update(|rvs| rvs.set_voltage(v));
    }

    pub fn set_resistance(&mut self, r: T) {
        self.update(|rvs| rvs.set_resistance(r));
    }
}

pub type CapVSHandle<T> = WDFHandle<T, CapacitiveVoltageSource<T>>;

impl<T: WDFloat> CapVSHandle<T> {
    #[inline(always)]
    pub fn set_voltage(&mut self, voltage: T) {
        self.update(|cvs| cvs.set_voltage(voltage));
    }

    pub fn set_capacitance(&mut self, capacitance: T) {
        self.update(|cvs| cvs.set_capacitance_value(capacitance));
    }
}

/// Safety: [`WDFHandle`](WDFHandle) is a typed non-owning pointer into a circuit tree. Moving the handle to
/// another thread does not create shared access on its own, and mutating through it still requires
/// `&mut self`. The crate assumes the owning circuit is not used re-entrantly. We keep this impl so
/// circuits containing handles can still be used in plugin/host code that requires `Send`.
unsafe impl<T, W> Send for WDFHandle<T, W>
where
    T: WDFloat,
    W: WDF<T>,
{
}
