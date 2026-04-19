//! Fluent API for building Wave Digital Filter trees.
//!
//! This module provides the `WDFBuilder` trait, which adds convenience methods
//! to all WDF components for connecting them in series or parallel.

use super::adaptors::{ParallelAdaptor, SeriesAdaptor};
use super::nonlinear::{Diode, DiodePair};
use super::roots::{IdealCurrentSource, IdealVoltageSource, Switch};
use super::traits::{WDFloat, WDF};
use crate::core::wdf::PolarityInverter;

/// A trait providing a builder interface for constructing WDF trees.
///
/// This trait is automatically implemented for all types that implement `WDF`.
pub trait WDFBuilder<T: WDFloat>: WDF<T> + Sized {
    /// Connects this component in series with another.
    fn series_with<W: WDF<T>>(self, p2: W) -> SeriesAdaptor<T, Self, W> {
        SeriesAdaptor::new(self, p2)
    }

    /// Sets the identifier for this component.
    fn id(self, id: &'static str) -> Self {
        self.params_mut().id = Some(id);
        self
    }

    /// Connects this component in parallel with another.
    fn parallel_with<W: WDF<T>>(self, p2: W) -> ParallelAdaptor<T, Self, W> {
        ParallelAdaptor::new(self, p2)
    }

    /// Attaches this component (or subtree) into an `IdealVoltageSource` root.
    fn to_ideal_voltage_source(self) -> IdealVoltageSource<T, Self> {
        IdealVoltageSource::new(self)
    }

    /// Wraps this component in a [`PolarityInverter`], negating all waves passing through it.
    fn polarity_inverted(self) -> PolarityInverter<T, Self> {
        PolarityInverter::new(self)
    }

    /// Attaches this component (or subtree) into an [`IdealCurrentSource`] root.
    fn to_ideal_current_source(self) -> IdealCurrentSource<T, Self> {
        IdealCurrentSource::new(self)
    }

    /// Attaches this component (or subtree) into a [`Switch`] root.
    fn to_switch(self, closed: bool) -> Switch<T, Self> {
        Switch::new(self, closed)
    }

    /// Attaches this component (or subtree) into a [`Diode`] root.
    fn to_diode(self, i_s: T, n_diodes: T) -> Diode<T, Self> {
        Diode::new(self, i_s, n_diodes)
    }

    /// Attaches this component (or subtree) into a [`DiodePair`] root.
    fn to_diode_pair(self, is: T) -> DiodePair<T, Self> {
        DiodePair::new(self, is)
    }
}

impl<T: WDFloat, W: WDF<T>> WDFBuilder<T> for W {}
