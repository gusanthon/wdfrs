//! Wave Digital Filter adaptors for connecting ports.
//!
//! Adaptors are the building blocks of WDF circuits, allowing components to be
//! connected in series or parallel while maintaining the wave-domain properties.

use super::traits::*;
use core::{any::Any, cell::UnsafeCell, ptr::NonNull};

/// A Series Adaptor for connecting two WDF components in series.
///
/// The series adaptor enforces that the current through both connected ports is equal.
/// In the wave domain, this leads to specific scattering equations for $a$ and $b$ waves.
pub struct SeriesAdaptor<T, P1, P2>
where
    T: WDFloat,
    P1: WDF<T>,
    P2: WDF<T>,
{
    params: UnsafeCell<WDFParams<T>>,
    /// The first child component.
    pub p1: P1,
    /// The second child component.
    pub p2: P2,
}

impl<T, P1, P2> SeriesAdaptor<T, P1, P2>
where
    T: WDFloat,
    P1: WDF<T>,
    P2: WDF<T>,
{
    /// Creates a new `SeriesAdaptor` connecting `p1` and `p2`.
    pub fn new(p1: P1, p2: P2) -> Self {
        let s = Self {
            params: UnsafeCell::new(WDFParams::default()),
            p1,
            p2,
        };
        s.calc_impedance();
        s
    }
}

impl<T, P1, P2> WDF<T> for SeriesAdaptor<T, P1, P2>
where
    T: WDFloat,
    P1: WDF<T>,
    P2: WDF<T>,
{
    #[inline(always)]
    fn params_ptr(&self) -> *mut WDFParams<T> {
        self.params.get()
    }

    /// Calculates the series impedance: $R_p = R_{p1} + R_{p2}$.
    fn calc_impedance(&self) {
        self.p1.calc_impedance();
        self.p2.calc_impedance();

        let p1_rp = self.p1.params().rp;
        let p2_rp = self.p2.params().rp;

        let p = self.params_mut();
        p.rp = p1_rp + p2_rp;
        p.g = T::one() / p.rp;
    }

    /// Calculates the reflected wave for the series connection: $b = -(b_1 + b_2)$.
    fn get_reflected_wave(&self) -> T {
        let b1 = self.p1.get_reflected_wave();
        let b2 = self.p2.get_reflected_wave();

        let b = -(b1 + b2);
        self.params_mut().b = b;
        b
    }

    /// Distributes the incident wave to children based on their relative impedances.
    fn set_incident_wave(&self, a: T) {
        let p = self.params_mut();
        p.a = a;

        let p1_b = self.p1.params().b;
        let p2_b = self.p2.params().b;
        let total_b = p1_b + p2_b;

        let r_total = p.rp;
        let p1_r = self.p1.params().rp;
        let p2_r = self.p2.params().rp;

        let common_term = (a + total_b) / r_total;
        let a1 = p1_b - p1_r * common_term;
        let a2 = p2_b - p2_r * common_term;

        self.p1.set_incident_wave(a1);
        self.p2.set_incident_wave(a2);
    }

    fn find_by_id_ptr(&self, id: &str) -> Option<NonNull<dyn Any>> {
        if self.params().id == Some(id) {
            let raw_ptr = self as *const Self as *mut Self as *mut dyn Any;
            return NonNull::new(raw_ptr);
        }

        self.p1
            .find_by_id_ptr(id)
            .or_else(|| self.p2.find_by_id_ptr(id))
    }

    fn recurse_print(&self, prefix: &str) {
        self.p1.print_tree(prefix, false);
        self.p2.print_tree(prefix, true);
    }

    fn for_each_child(&self, f: &mut dyn FnMut(&dyn WDF<T>)) {
        f(&self.p1);
        f(&self.p2);
    }

    fn set_sample_rate(&self, fs: T) {
        self.for_each_child(&mut |child| {
            child.set_sample_rate(fs);
        });
        self.calc_impedance();
    }
}

//====================================================================================

struct ParAdapInternal<T: WDFloat> {
    pub params: WDFParams<T>,
    pub p1_reflect: T,
    pub b_diff: T,
    pub b_temp: T,
}

/// A Parallel Adaptor for connecting two WDF components in parallel.
///
/// The parallel adaptor enforces that the voltage across both connected ports is equal.
pub struct ParallelAdaptor<T, P1, P2>
where
    T: WDFloat,
    P1: WDF<T>,
    P2: WDF<T>,
{
    /// The first child component.
    pub p1: P1,
    /// The second child component.
    pub p2: P2,
    inner: UnsafeCell<ParAdapInternal<T>>,
}

impl<T, P1, P2> ParallelAdaptor<T, P1, P2>
where
    T: WDFloat,
    P1: WDF<T>,
    P2: WDF<T>,
{
    /// Creates a new `ParallelAdaptor` connecting `p1` and `p2`.
    pub fn new(p1: P1, p2: P2) -> Self {
        let p = Self {
            p1,
            p2,
            inner: ParAdapInternal {
                params: WDFParams::default(),
                p1_reflect: T::zero(),
                b_diff: T::zero(),
                b_temp: T::zero(),
            }
            .into(),
        };
        p.calc_impedance();
        p
    }

    #[inline(always)]
    fn inner_mut(&self) -> &mut ParAdapInternal<T> {
        unsafe { &mut *self.inner.get() }
    }
}

impl<T, P1, P2> WDF<T> for ParallelAdaptor<T, P1, P2>
where
    T: WDFloat,
    P1: WDF<T>,
    P2: WDF<T>,
{
    #[inline(always)]
    fn params_ptr(&self) -> *mut WDFParams<T> {
        unsafe { &mut (*self.inner.get()).params }
    }

    /// Calculates the parallel conductance: $G_p = G_{p1} + G_{p2}$.
    fn calc_impedance(&self) {
        self.p1.calc_impedance();
        self.p2.calc_impedance();

        let g1 = self.p1.params().g;
        let g2 = self.p2.params().g;

        let i = self.inner_mut();
        i.params.g = g1 + g2;
        i.params.rp = T::one() / i.params.g;
        i.p1_reflect = g1 / i.params.g;
    }

    /// Calculates the reflected wave for the parallel connection.
    fn get_reflected_wave(&self) -> T {
        let b1 = self.p1.get_reflected_wave();
        let b2 = self.p2.get_reflected_wave();

        let i = self.inner_mut();
        i.b_diff = b2 - b1;
        i.b_temp = -i.p1_reflect * i.b_diff;

        let b = b2 + i.b_temp;
        i.params.b = b;
        b
    }

    /// Distributes the incident wave to children.
    fn set_incident_wave(&self, a: T) {
        let i = self.inner_mut();
        i.params.a = a;

        let b_common = a + i.b_temp;

        self.p1.set_incident_wave(b_common + i.b_diff);
        self.p2.set_incident_wave(b_common);
    }

    fn find_by_id_ptr(&self, id: &str) -> Option<NonNull<dyn Any>> {
        if self.params().id == Some(id) {
            let raw_ptr = self as *const Self as *mut Self as *mut dyn Any;
            return NonNull::new(raw_ptr);
        }

        self.p1
            .find_by_id_ptr(id)
            .or_else(|| self.p2.find_by_id_ptr(id))
    }

    fn recurse_print(&self, prefix: &str) {
        self.p1.print_tree(prefix, false);
        self.p2.print_tree(prefix, true);
    }

    fn for_each_child(&self, f: &mut dyn FnMut(&dyn WDF<T>)) {
        f(&self.p1);
        f(&self.p2);
    }

    fn set_sample_rate(&self, fs: T) {
        self.for_each_child(&mut |child| {
            child.set_sample_rate(fs);
        });
        self.calc_impedance();
    }
}

/// A Polarity Inverter for flipping the phase of a signal in a WDF tree.
pub struct PolarityInverter<T, P1>
where
    T: WDFloat,
    P1: WDF<T>,
{
    p1: P1,
    params: UnsafeCell<WDFParams<T>>,
}

impl<T: WDFloat, P1: WDF<T>> PolarityInverter<T, P1> {
    /// Creates a new `PolarityInverter` for component `p1`.
    pub fn new(p1: P1) -> Self {
        let inverter = Self {
            p1,
            params: UnsafeCell::new(WDFParams::default()),
        };
        inverter.calc_impedance();
        inverter
    }
}

impl<T, P1> WDF<T> for PolarityInverter<T, P1>
where
    T: WDFloat,
    P1: WDF<T>,
{
    #[inline(always)]
    fn params_ptr(&self) -> *mut WDFParams<T> {
        self.params.get()
    }

    fn calc_impedance(&self) {
        self.p1.calc_impedance();

        let p1_rp = self.p1.params().rp;
        let p = self.params_mut();
        p.rp = p1_rp;
        p.g = T::one() / p.rp;
    }

    fn get_reflected_wave(&self) -> T {
        let b = T::zero() - self.p1.get_reflected_wave();
        self.params_mut().b = b;
        b
    }

    fn set_incident_wave(&self, a: T) {
        self.params_mut().a = a;
        self.p1.set_incident_wave(-a);
    }

    fn find_by_id_ptr(&self, id: &str) -> Option<NonNull<dyn Any>> {
        if self.params().id == Some(id) {
            let raw_ptr = self as *const Self as *mut Self as *mut dyn Any;
            return NonNull::new(raw_ptr);
        }

        self.p1.find_by_id_ptr(id)
    }

    fn recurse_print(&self, prefix: &str) {
        self.p1.print_tree(prefix, true);
    }

    fn for_each_child(&self, f: &mut dyn FnMut(&dyn WDF<T>)) {
        f(&self.p1);
    }

    fn set_sample_rate(&self, fs: T) {
        self.for_each_child(&mut |child| {
            child.set_sample_rate(fs);
        });
        self.calc_impedance();
    }
}
