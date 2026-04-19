use wdfrs::circuits::{
    bassman::new_bassman_tone_stack, baxandall::new_baxandall, diode_clipper::new_diode_clipper,
    ff2::new_ff2, lpf2::new_lpf2,
};
use wdfrs::core::bench::compare_circuits;

fn main() {
    let fs = 192e3_f32;
    let n_seconds = 100.0;

    let mut lpf2 = new_lpf2(fs);
    let mut dc = new_diode_clipper(fs, 1e3, 1.0);
    let mut baxandall = new_baxandall(fs);
    let mut bassman = new_bassman_tone_stack(fs, 0.5, 0.5, 0.5);
    let mut ff2 = new_ff2(fs);

    compare_circuits(
        vec![
            &mut *lpf2,
            &mut *dc,
            &mut *baxandall,
            &mut *bassman,
            &mut *ff2,
        ],
        n_seconds,
        fs,
    );
}
