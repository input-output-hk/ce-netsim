//! Internal Karney geodesic implementation.
//!
//! This module vendors and trims production code from `geographiclib-rs`
//! (MIT license), itself a Rust port of GeographicLib geodesic algorithms.
//! We keep it in-tree to avoid adding an external dependency while providing
//! robust inverse geodesic computation for near-antipodal coordinates.

#![allow(non_snake_case)]
#![allow(clippy::excessive_precision)]

use super::geodesic::{CARR_SIZE, GEODESIC_ORDER};

pub const DIGITS: u64 = 53;

/// Returns `x²`.
pub fn sq(x: f64) -> f64 {
    x.powi(2)
}

/// Normalise a two-vector `(x, y)` in-place so that `hypot(x, y) == 1`.
pub fn norm(x: &mut f64, y: &mut f64) {
    let r = x.hypot(*y);
    *x /= r;
    *y /= r;
}

/// Error-free transformation of a sum: returns `(s, t)` where `s = u + v`
/// and `t` is the rounding error such that `s + t == u + v` exactly.
pub fn sum(u: f64, v: f64) -> (f64, f64) {
    let s = u + v;
    let up = s - v;
    let vpp = s - up;
    let up = up - u;
    let vpp = vpp - v;
    let t = -(up + vpp);
    (s, t)
}

/// Evaluate the degree-`n` polynomial whose coefficients are `p[0..=n]`
/// at `x` using Horner's method. `p` must have at least `n + 1` elements.
pub fn polyval(n: usize, p: &[f64], x: f64) -> f64 {
    debug_assert!(
        p.len() > n,
        "polyval: slice too short (len={}, n={})",
        p.len(),
        n
    );
    let mut y = p[0];
    for val in &p[1..=n] {
        y = y * x + val;
    }
    y
}

/// Round an angle (in degrees) so that small values underflow to exactly 0,
/// avoiding near-singular cases in the geodesic solver.
pub fn ang_round(x: f64) -> f64 {
    // The makes the smallest gap in x = 1/16 - nextafter(1/16, 0) = 1/2^57
    // for reals = 0.7 pm on the earth if x is an angle in degrees.  (This
    // is about 1000 times more resolution than we get with angles around 90
    // degrees.)  We use this to avoid having to deal with near singular
    // cases when x is non-zero but tiny (e.g., 1.0e-200).
    let z = 1.0 / 16.0;
    let mut y = x.abs();
    // The compiler mustn't "simplify" z - (z - y) to y
    if y < z {
        y = z - (z - y);
    };
    if x == 0.0 {
        0.0
    } else if x < 0.0 {
        -y
    } else {
        y
    }
}

// remainder of x/y in the range [-y/2, y/2]
fn remainder(x: f64, y: f64) -> f64 {
    // z = math.fmod(x, y) if Math.isfinite(x) else Math.nan
    let z = if x.is_finite() { x % y } else { f64::NAN };

    // # On Windows 32-bit with python 2.7, math.fmod(-0.0, 360) = +0.0
    // # This fixes this bug.  See also Math::AngNormalize in the C++ library.
    // # sincosd has a similar fix.
    // z = x if x == 0 else z
    let z = if x == 0.0 { x } else { z };

    // return (z + y if z < -y/2 else
    // (z if z < y/2 else z -y))
    if z < -y / 2.0 {
        z + y
    } else if z < y / 2.0 {
        z
    } else {
        z - y
    }
}

/// Reduce an angle in degrees to the range `(-180, 180]`.
pub fn ang_normalize(x: f64) -> f64 {
    // y = Math.remainder(x, 360)
    // return 180 if y == -180 else y
    let y = remainder(x, 360.0);
    if y == -180.0 { 180.0 } else { y }
}

/// Replace latitudes outside `[-90, 90]` degrees with `NaN`.
pub fn lat_fix(x: f64) -> f64 {
    if x.abs() > 90.0 { f64::NAN } else { x }
}

/// Compute `y − x` in degrees, reduced accurately to `(-180, 180]`.
///
/// Returns an error-free `(difference, rounding_error)` pair via [`sum`].
pub fn ang_diff(x: f64, y: f64) -> (f64, f64) {
    let (d, t) = sum(ang_normalize(-x), ang_normalize(y));
    let d = ang_normalize(d);
    if d == 180.0 && t > 0.0 {
        sum(-180.0, t)
    } else {
        sum(d, t)
    }
}

/// Compute `(sin x, cos x)` for `x` given in degrees, with exact values at
/// multiples of 90°.
pub fn sincosd(x: f64) -> (f64, f64) {
    // Keep this dependency-free: reduce by quarter turns manually.
    // For our use here, standard floating reduction is sufficient.
    let mut q = (x / 90.0).round();
    let mut r = x - 90.0 * q;
    if r <= -45.0 {
        r += 90.0;
        q -= 1.0;
    } else if r > 45.0 {
        r -= 90.0;
        q += 1.0;
    }
    let q = q as i32;

    r = r.to_radians();

    let (mut sinx, mut cosx) = r.sin_cos();

    (sinx, cosx) = match q as u32 & 3 {
        0 => (sinx, cosx),
        1 => (cosx, -sinx),
        2 => (-sinx, -cosx),
        3 => (-cosx, sinx),
        _ => unreachable!(),
    };

    // special values from F.10.1.12
    cosx += 0.0;

    // special values from F.10.1.13
    if sinx == 0.0 {
        sinx = sinx.copysign(x);
    }
    (sinx, cosx)
}

/// Compute `atan2(y, x)` with the result in degrees, handling quadrant
/// reduction carefully to preserve sign at ±0 and ±180.
#[allow(dead_code)]
pub(crate) fn atan2d(mut y: f64, mut x: f64) -> f64 {
    let mut q = if y.abs() > x.abs() {
        std::mem::swap(&mut x, &mut y);
        2
    } else {
        0
    };
    if x < 0.0 {
        q += 1;
        x = -x;
    }
    let mut ang = y.atan2(x).to_degrees();
    match q {
        0 => {}
        1 => {
            ang = if y >= 0.0 { 180.0 - ang } else { -180.0 - ang };
        }
        2 => {
            ang = 90.0 - ang;
        }
        3 => {
            ang += -90.0;
        }
        _ => unreachable!(),
    };
    ang
}

/// Compute `e·atanh(e·x)` for an ellipsoid with (signed) eccentricity `es`.
///
/// For a prolate spheroid (`es < 0`) uses `atan` instead of `atanh`.
pub fn eatanhe(x: f64, es: f64) -> f64 {
    if es > 0.0 {
        es * (es * x).atanh()
    } else {
        -es * (es * x).atan()
    }
}

/// Evaluate a trigonometric series using Clenshaw summation.
///
/// If `sinp` is `true` the series is multiplied by `sin x`; otherwise by
/// `cos x`. `c` contains the Fourier coefficients.
pub fn sin_cos_series<const N: usize>(sinp: bool, sinx: f64, cosx: f64, c: &[f64; N]) -> f64 {
    let mut k = c.len();
    let mut n: i64 = k as i64 - if sinp { 1 } else { 0 };
    let ar: f64 = 2.0 * (cosx - sinx) * (cosx + sinx);
    let mut y1 = 0.0;
    let mut y0: f64 = if n & 1 != 0 {
        k -= 1;
        c[k]
    } else {
        0.0
    };
    n /= 2;
    while n > 0 {
        n -= 1;
        k -= 1;
        y1 = ar * y0 - y1 + c[k];
        k -= 1;
        y0 = ar * y1 - y0 + c[k];
    }
    if sinp {
        2.0 * sinx * cosx * y0
    } else {
        cosx * (y0 - y1)
    }
}

/// Solve the astroid equation used in the geodesic near-antipodal case.
///
/// Returns the positive root of `x²·k⁴ + 2·x·k³ − (y² − 1)·k² − 2·y²·k − y² = 0`.
pub fn astroid(x: f64, y: f64) -> f64 {
    let p = sq(x);
    let q = sq(y);
    let r = (p + q - 1.0) / 6.0;
    if !(q == 0.0 && r <= 0.0) {
        let s = p * q / 4.0;
        let r2 = sq(r);
        let r3 = r * r2;
        let disc = s * (s + 2.0 * r3);
        let mut u = r;
        if disc >= 0.0 {
            let mut t3 = s + r3;
            t3 += if t3 < 0.0 { -disc.sqrt() } else { disc.sqrt() };
            let t = t3.cbrt();
            u += t + if t != 0.0 { r2 / t } else { 0.0 };
        } else {
            let ang = (-disc).sqrt().atan2(-(s + r3));
            u += 2.0 * r * (ang / 3.0).cos();
        }
        let v = (sq(u) + q).sqrt();
        let uv = if u < 0.0 { q / (v - u) } else { u + v };
        let w = (uv - q) / (2.0 * v);
        uv / ((uv + sq(w)).sqrt() + w)
    } else {
        0.0
    }
}

/// Compute `A₁(ε) − 1`, the scale factor for the first-order series. Ported from GeographicLib.
pub fn _A1m1f(eps: f64) -> f64 {
    const COEFF: [f64; 5] = [1.0, 4.0, 64.0, 0.0, 256.0];
    let m = GEODESIC_ORDER / 2;
    let t = polyval(m, &COEFF, sq(eps)) / COEFF[m + 1];
    (t + eps) / (1.0 - eps)
}

/// Compute the `C₁` series coefficients into `c`. Ported from GeographicLib.
pub fn _C1f(eps: f64, c: &mut [f64; CARR_SIZE]) {
    const COEFF: [f64; 18] = [
        -1.0, 6.0, -16.0, 32.0, -9.0, 64.0, -128.0, 2048.0, 9.0, -16.0, 768.0, 3.0, -5.0, 512.0,
        -7.0, 1280.0, -7.0, 2048.0,
    ];
    let eps2 = sq(eps);
    let mut d = eps;
    let mut o = 0;
    // Clippy wants us to turn this into `c.iter_mut().enumerate().take(geodesic_order + 1).skip(1)`
    // but benching (rust-1.75) shows that it would be slower.
    #[allow(clippy::needless_range_loop)]
    for l in 1..=GEODESIC_ORDER {
        let m = (GEODESIC_ORDER - l) / 2;
        c[l] = d * polyval(m, &COEFF[o..], eps2) / COEFF[o + m + 1];
        o += m + 2;
        d *= eps;
    }
}

/// Compute the `C₁'` (inverse) series coefficients into `c`. Ported from GeographicLib.
pub fn _C1pf(eps: f64, c: &mut [f64; CARR_SIZE]) {
    const COEFF: [f64; 18] = [
        205.0, -432.0, 768.0, 1536.0, 4005.0, -4736.0, 3840.0, 12288.0, -225.0, 116.0, 384.0,
        -7173.0, 2695.0, 7680.0, 3467.0, 7680.0, 38081.0, 61440.0,
    ];
    let eps2 = sq(eps);
    let mut d = eps;
    let mut o = 0;
    // Clippy wants us to turn this into `c.iter_mut().enumerate().take(geodesic_order + 1).skip(1)`
    // but benching (rust-1.75) shows that it would be slower.
    #[allow(clippy::needless_range_loop)]
    for l in 1..=GEODESIC_ORDER {
        let m = (GEODESIC_ORDER - l) / 2;
        c[l] = d * polyval(m, &COEFF[o..], eps2) / COEFF[o + m + 1];
        o += m + 2;
        d *= eps;
    }
}

/// Compute `A₂(ε) − 1`, the scale factor for the second-order series. Ported from GeographicLib.
pub fn _A2m1f(eps: f64) -> f64 {
    const COEFF: [f64; 5] = [-11.0, -28.0, -192.0, 0.0, 256.0];
    let m = GEODESIC_ORDER / 2;
    let t = polyval(m, &COEFF, sq(eps)) / COEFF[m + 1];
    (t - eps) / (1.0 + eps)
}

/// Compute the `C₂` series coefficients into `c`. Ported from GeographicLib.
pub fn _C2f(eps: f64, c: &mut [f64; CARR_SIZE]) {
    const COEFF: [f64; 18] = [
        1.0, 2.0, 16.0, 32.0, 35.0, 64.0, 384.0, 2048.0, 15.0, 80.0, 768.0, 7.0, 35.0, 512.0, 63.0,
        1280.0, 77.0, 2048.0,
    ];
    let eps2 = sq(eps);
    let mut d = eps;
    let mut o = 0;
    // Clippy wants us to turn this into `c.iter_mut().enumerate().take(geodesic_order + 1).skip(1)`
    // but benching (rust-1.75) shows that it would be slower.
    #[allow(clippy::needless_range_loop)]
    for l in 1..=GEODESIC_ORDER {
        let m = (GEODESIC_ORDER - l) / 2;
        c[l] = d * polyval(m, &COEFF[o..], eps2) / COEFF[o + m + 1];
        o += m + 2;
        d *= eps;
    }
}
