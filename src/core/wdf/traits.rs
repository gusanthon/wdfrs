//! Core WDF traits: [`WDF`], [`RootWDF`], [`WDFloat`], and the [`WDFParams`] data struct.

use crate::core::{utils::*, wdf::WDFHandle};
use core::{
    any::Any,
    fmt::{Debug, Display},
    iter::Sum,
    ptr::NonNull,
};
use num_traits::{Float, FromPrimitive, Signed};

/// Abstraction for floating-point types used in WDF calculations.
///
/// This trait ensures the numeric type supports all necessary operations for
/// signal processing, including basic arithmetic, trigonometric functions,
/// and conversion from primitives.
pub trait WDFloat:
    Sync + Send + FromPrimitive + Signed + Float + Default + Debug + Display + Sum + Unpin + 'static
{
}

impl<T> WDFloat for T where
    T: Sync
        + Send
        + FromPrimitive
        + Signed
        + Float
        + Default
        + Debug
        + Display
        + Sum
        + Unpin
        + 'static
{
}

/// Physical parameters of a WDF port.
///
/// In Wave Digital Filters, signals are represented as waves (`a` and `b`)
/// rather than voltages and currents. This struct holds the wave values
/// and the port impedance/conductance.
#[derive(Default, Debug)]
pub struct WDFParams<T: WDFloat> {
    /// Port Resistance ($R_p$).
    pub rp: T,
    /// Port Conductance ($G = 1/R_p$).
    pub g: T,
    /// Incident Wave ($a$). Represents the wave entering the port.
    pub a: T,
    /// Reflected Wave ($b$). Represents the wave leaving the port.
    pub b: T,
    /// Optional identifier for the component (useful for debugging and handle lookup).
    pub id: Option<&'static str>,
    /// Optional sampling frequency ($f_s$). Used by frequency-dependent components like Capacitors/Inductors.
    pub fs: Option<T>,
}

impl<T: WDFloat> WDFParams<T> {
    /// Resets wave state (a, b) to zero. Preserves impedance, id, and fs.
    pub fn reset(&mut self) {
        self.a = T::zero();
        self.b = T::zero();
    }
}

/// The core trait for all Wave Digital Filter components.
///
/// Any component in a WDF tree (Resistors, Capacitors, Adaptors, Sources) must implement this trait.
/// It defines the standard interface for impedance calculation and wave propagation.
pub trait WDF<T: WDFloat>: Unpin + Send + 'static {
    /// Returns a raw mutable pointer to the internal [`WDFParams`].
    ///
    /// Used internally for mutable access through `&self` during tree traversal.
    /// Must be backed by an [`UnsafeCell`](std::cell::UnsafeCell). Sound only when
    /// the tree is single-threaded, acyclic, and `compute()` is not re-entrant.
    fn params_ptr(&self) -> *mut WDFParams<T>;

    /// Calculates the port impedance for this component.
    ///
    /// This is typically called once during initialization or whenever a component
    /// value (like capacitance) changes. In a tree, this propagates from leaves up to the root.
    fn calc_impedance(&self);

    /// Calculates and returns the reflected wave ($b$) at this port.
    ///
    /// Part of the upward traversal of the WDF tree.
    fn get_reflected_wave(&self) -> T;

    /// Sets the incident wave ($a$) for this component and propagates it to children if applicable.
    ///
    /// Part of the downward traversal of the WDF tree.
    fn set_incident_wave(&self, a: T);

    /// Accesses the `WDFParams` for this component.
    #[inline(always)]
    fn params(&self) -> &WDFParams<T> {
        unsafe { &*self.params_ptr() }
    }

    /// Accesses the `WDFParams` for this component mutably.
    #[inline(always)]
    fn params_mut(&self) -> &mut WDFParams<T> {
        unsafe { &mut *self.params_ptr() }
    }

    /// Converts the current wave states ($a$, $b$) to a physical voltage.
    ///
    /// Formula: $V = (a + b) / 2$
    #[inline(always)]
    fn wave_to_voltage(&self) -> T {
        let p = self.params();
        (p.a + p.b) / num_t(2.0)
    }

    /// Converts the current wave states ($a$, $b$) to a physical current.
    ///
    /// Formula: $I = (a - b) / (2 \cdot R_p)$
    #[inline(always)]
    fn wave_to_current(&self) -> T {
        let p = self.params();
        (p.a - p.b) / (num_t::<T>(2.0) * p.rp)
    }

    /// Resets the component state.
    fn reset(&self) {
        self.params_mut().reset();
    }

    /// Sets the voltage for source components.
    ///
    /// # Panics
    /// Panics if called on a component that is not a voltage source.
    fn set_voltage(&self, _voltage: T) {
        unimplemented!()
    }

    /// Sets the current for source components.
    ///
    /// # Panics
    /// Panics if called on a component that is not a current source.
    fn set_current(&self, _current: T) {
        unimplemented!()
    }

    /// Internal mechanism to print the structure of the WDF tree from this node.
    fn print_tree(&self, prefix: &str, is_last: bool)
    where
        Self: Sized,
    {
        let id_str = if let Some(id) = self.params().id {
            format!(" ({})", id)
        } else {
            "".to_string()
        };

        let fs_str = if let Some(fs) = self.params().fs {
            format!(", ({} kHz fs)", fs / num_t(1e3))
        } else {
            "".to_string()
        };

        let marker = if is_last { "└── " } else { "├── " };

        println!(
            "{}{}{}{}, Rp: {:.2}{}",
            prefix,
            marker,
            type_of::<Self>(),
            id_str,
            self.params().rp,
            fs_str,
        );

        let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
        self.recurse_print(&new_prefix);
    }

    /// Internal mechanism to find a node in the tree by its ID and returns a raw pointer to it as `Any`.
    fn find_by_id_ptr(&self, id: &str) -> Option<NonNull<dyn Any>>
    where
        Self: Sized,
    {
        let self_ptr = self as *const Self;
        unsafe {
            if (*self_ptr).params().id == Some(id) {
                return Some(NonNull::new_unchecked(
                    self_ptr as *mut Self as *mut dyn Any,
                ));
            }
        }
        None
    }

    /// Propagates a new sample rate down the tree and recalculates impedances.
    fn set_sample_rate(&self, fs: T) {
        self.for_each_child(&mut |child| {
            child.set_sample_rate(fs);
        });

        if self.params().fs.is_some() {
            self.params_mut().fs = Some(fs);
        }

        self.calc_impedance();
    }

    /// Applies the given function to each child of this node in the WDF tree.
    ///
    /// This is a generic tree traversal helper that allows you to visit each child.
    /// The default implementation does nothing (for leaf nodes and components with no children).
    fn for_each_child(&self, _f: &mut dyn FnMut(&dyn WDF<T>)) {
        // Default: no children
    }

    /// Visits all descendants (children, grandchildren, …) of this node recursively.
    /// Does NOT visit `self` — call `f(self)` separately if needed.
    fn walk_children(&self, f: &mut dyn FnMut(&dyn WDF<T>)) {
        self.for_each_child(&mut |child| {
            f(child);
            child.walk_children(f);
        });
    }

    /// Internal method for recursive tree printing.
    fn recurse_print(&self, _prefix: &str) {}
}

/// Trait for the root node of a WDF circuit.
///
/// The root node is responsible for initiating the wave propagation cycle.
pub trait RootWDF<T: WDFloat>: WDF<T> {
    type Next: WDF<T>;

    /// Returns the component connected to the root.
    fn next(&self) -> &Self::Next;

    /// Performs one full WDF time step.
    ///
    /// This involves:
    /// 1. Getting the reflected wave from the connected subtree.
    /// 2. Setting it as the incident wave for the root.
    /// 3. Getting the reflected wave from the root.
    /// 4. Setting it as the incident wave for the connected subtree.
    ///
    /// This should be called after setting the voltage of the input source,
    /// and before reading the output voltage/current.
    #[inline(always)]
    fn compute(&self) {
        let next_b = self.next().get_reflected_wave();
        self.set_incident_wave(next_b);
        let self_b = self.get_reflected_wave();
        self.next().set_incident_wave(self_b);
    }

    /// Sets the source voltage for the root.
    #[inline(always)]
    fn set_root_source_voltage(&self, v_in: T) {
        self.set_voltage(v_in);
        self.compute();
    }

    /// Visits every node in the tree (root included) with the given closure.
    fn for_each_tree(&self, f: &mut dyn FnMut(&dyn WDF<T>))
    where
        Self: Sized,
    {
        f(self);
        f(self.next());
        self.next().walk_children(f);
    }

    fn set_tree_sample_rate(&self, fs: T) {
        self.next().set_sample_rate(fs);
    }

    fn reset_tree(&self)
    where
        Self: Sized,
    {
        self.for_each_tree(&mut |wdf| wdf.reset());
    }

    fn print_full_tree(&self)
    where
        Self: Sized,
    {
        let id_str = if let Some(id) = self.params().id {
            format!(" ({})", id)
        } else {
            "".to_string()
        };
        let type_name = type_of::<Self>();

        println!("{}{}, Rp: {:.2}", type_name, id_str, self.params().rp);
        self.recurse_print("");
        println!("\n");
    }

    /// Returns a typed handle to the component with the given ID.
    ///
    /// # Panics
    /// Panics if the ID is not found or if the type doesn't match `W`.
    #[doc(hidden)]
    fn get_handle_for_id<W: WDF<T>>(&self, id: &str) -> WDFHandle<T, W>
    where
        Self: Sized,
    {
        let any_ptr = self
            .find_by_id_ptr(id)
            .unwrap_or_else(|| panic!("WDF ID {} not found!", id));

        unsafe {
            let any_raw = any_ptr.as_ptr();
            if (*any_raw).is::<W>() {
                WDFHandle::new(NonNull::new_unchecked(any_raw as *mut W))
            } else {
                panic!("Type mismatch");
            }
        }
    }
}
