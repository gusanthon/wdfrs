#![feature(type_alias_impl_trait)]
#![doc = include_str!("../README.md")]

pub mod circuits;

#[allow(clippy::mut_from_ref)]
pub mod core;
pub use core::circuit::WDFCircuit;
pub use wdf_macros::{define_root_rtype, define_rtype, export_wdf, WDFCircuit};

#[cfg(test)]
mod tests {
    use crate::circuits::{
        bassman::new_bassman_tone_stack, baxandall::new_baxandall,
        diode_clipper::new_diode_clipper, ff2::new_ff2, lpf2::new_lpf2,
    };
    use crate::core::circuit::WDFCircuit;

    #[test]
    fn lpf2() {
        let mut c = new_lpf2(48e3_f64);
        let _ = c.process(1.0);
    }

    #[test]
    fn diode_clipper() {
        let mut c = new_diode_clipper(48e3_f64, 1e3, 0.0);
        let _ = c.process(1.0);
    }

    #[test]
    fn baxandall() {
        let mut c = new_baxandall(48e3_f64);
        let _ = c.process(1.0);
    }

    #[test]
    fn bassman() {
        let mut c = new_bassman_tone_stack(48e3_f64, 0.5, 0.5, 0.5);
        let _ = c.process(1.0);
    }

    #[test]
    fn ff2() {
        let mut c = new_ff2(48e3_f64);
        let _ = c.process(1.0);
    }
}
