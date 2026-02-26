//! Internal Karney geodesic implementation.
//!
//! This module vendors and trims production code from `geographiclib-rs`
//! (MIT license), itself a Rust port of GeographicLib geodesic algorithms.
//! We keep it in-tree to avoid adding an external dependency while providing
//! robust inverse geodesic computation for near-antipodal coordinates.

#![allow(non_snake_case)]
#![allow(clippy::excessive_precision)]

use super::geomath;

use std::f64::consts::{FRAC_1_SQRT_2, PI};

const CAP_C1: u64 = 1 << 0;
const CAP_C2: u64 = 1 << 2;
const CAP_C4: u64 = 1 << 4;
const OUT_MASK: u64 = 0xFF80;
const EMPTY: u64 = 0;
const DISTANCE: u64 = (1 << 10) | CAP_C1;
const REDUCEDLENGTH: u64 = (1 << 12) | CAP_C1 | CAP_C2;
const GEODESICSCALE: u64 = (1 << 13) | CAP_C1 | CAP_C2;
const AREA: u64 = (1 << 14) | CAP_C4;

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
pub struct Geodesic {
    pub a: f64,
    pub f: f64,
    pub _f1: f64,
    pub _e2: f64,
    pub _ep2: f64,
    _n: f64,
    pub _b: f64,
    pub _c2: f64,
    _etol2: f64,
    _A3x: [f64; GEODESIC_ORDER],
    _C3x: [f64; _nC3x_],
    _C4x: [f64; _nC4x_],

    _nC3x_: usize,
    _nC4x_: usize,
    maxit1_: u64,
    maxit2_: u64,

    pub tiny_: f64,
    tol0_: f64,
    tol1_: f64,
    _tol2_: f64,
    tolb_: f64,
    xthresh_: f64,
}

const COEFF_A3: [f64; 18] = [
    -3.0, 128.0, -2.0, -3.0, 64.0, -1.0, -3.0, -1.0, 16.0, 3.0, -1.0, -2.0, 8.0, 1.0, -1.0, 2.0,
    1.0, 1.0,
];

const COEFF_C3: [f64; 45] = [
    3.0, 128.0, 2.0, 5.0, 128.0, -1.0, 3.0, 3.0, 64.0, -1.0, 0.0, 1.0, 8.0, -1.0, 1.0, 4.0, 5.0,
    256.0, 1.0, 3.0, 128.0, -3.0, -2.0, 3.0, 64.0, 1.0, -3.0, 2.0, 32.0, 7.0, 512.0, -10.0, 9.0,
    384.0, 5.0, -9.0, 5.0, 192.0, 7.0, 512.0, -14.0, 7.0, 512.0, 21.0, 2560.0,
];

const COEFF_C4: [f64; 77] = [
    97.0, 15015.0, 1088.0, 156.0, 45045.0, -224.0, -4784.0, 1573.0, 45045.0, -10656.0, 14144.0,
    -4576.0, -858.0, 45045.0, 64.0, 624.0, -4576.0, 6864.0, -3003.0, 15015.0, 100.0, 208.0, 572.0,
    3432.0, -12012.0, 30030.0, 45045.0, 1.0, 9009.0, -2944.0, 468.0, 135135.0, 5792.0, 1040.0,
    -1287.0, 135135.0, 5952.0, -11648.0, 9152.0, -2574.0, 135135.0, -64.0, -624.0, 4576.0, -6864.0,
    3003.0, 135135.0, 8.0, 10725.0, 1856.0, -936.0, 225225.0, -8448.0, 4992.0, -1144.0, 225225.0,
    -1440.0, 4160.0, -4576.0, 1716.0, 225225.0, -136.0, 63063.0, 1024.0, -208.0, 105105.0, 3584.0,
    -3328.0, 1144.0, 315315.0, -128.0, 135135.0, -2560.0, 832.0, 405405.0, 128.0, 99099.0,
];

pub const GEODESIC_ORDER: usize = 6;
pub(crate) const CARR_SIZE: usize = GEODESIC_ORDER + 1;

#[allow(non_upper_case_globals)]
const _nC3x_: usize = 15;
#[allow(non_upper_case_globals)]
const _nC4x_: usize = 21;

impl Geodesic {
    pub fn new(a: f64, f: f64) -> Self {
        let maxit1_ = 20;
        let maxit2_ = maxit1_ + geomath::DIGITS + 10;
        let tiny_ = f64::MIN_POSITIVE.sqrt();
        let tol0_ = f64::EPSILON;
        let tol1_ = 200.0 * tol0_;
        let _tol2_ = tol0_.sqrt();
        let tolb_ = tol0_ * _tol2_;
        let xthresh_ = 1000.0 * _tol2_;

        let _f1 = 1.0 - f;
        let _e2 = f * (2.0 - f);
        let _ep2 = _e2 / geomath::sq(_f1);
        let _n = f / (2.0 - f);
        let _b = a * _f1;
        let _c2 = (geomath::sq(a)
            + geomath::sq(_b)
                * (if _e2 == 0.0 {
                    1.0
                } else {
                    geomath::eatanhe(1.0, (if f < 0.0 { -1.0 } else { 1.0 }) * _e2.abs().sqrt())
                        / _e2
                }))
            / 2.0;
        let _etol2 = 0.1 * _tol2_ / (f.abs().max(0.001) * (1.0 - f / 2.0).min(1.0) / 2.0).sqrt();

        let mut _A3x: [f64; GEODESIC_ORDER] = [0.0; GEODESIC_ORDER];
        let mut _C3x: [f64; _nC3x_] = [0.0; _nC3x_];
        let mut _C4x: [f64; _nC4x_] = [0.0; _nC4x_];

        // Call a3coeff
        let mut o: usize = 0;
        for (k, j) in (0..GEODESIC_ORDER).rev().enumerate() {
            let m = j.min(GEODESIC_ORDER - j - 1);
            _A3x[k] = geomath::polyval(m, &COEFF_A3[o..], _n) / COEFF_A3[o + m + 1];
            o += m + 2;
        }

        // c3coeff
        let mut o = 0;
        let mut k = 0;
        for l in 1..GEODESIC_ORDER {
            for j in (l..GEODESIC_ORDER).rev() {
                let m = j.min(GEODESIC_ORDER - j - 1);
                _C3x[k] = geomath::polyval(m, &COEFF_C3[o..], _n) / COEFF_C3[o + m + 1];
                k += 1;
                o += m + 2;
            }
        }

        // c4coeff
        let mut o = 0;
        let mut k = 0;
        for l in 0..GEODESIC_ORDER {
            for j in (l..GEODESIC_ORDER).rev() {
                let m = GEODESIC_ORDER - j - 1;
                _C4x[k] = geomath::polyval(m, &COEFF_C4[o..], _n) / COEFF_C4[o + m + 1];
                k += 1;
                o += m + 2;
            }
        }

        Geodesic {
            a,
            f,
            _f1,
            _e2,
            _ep2,
            _n,
            _b,
            _c2,
            _etol2,
            _A3x,
            _C3x,
            _C4x,

            _nC3x_,
            _nC4x_,
            maxit1_,
            maxit2_,

            tiny_,
            tol0_,
            tol1_,
            _tol2_,
            tolb_,
            xthresh_,
        }
    }

    pub fn _A3f(&self, eps: f64) -> f64 {
        geomath::polyval(GEODESIC_ORDER - 1, &self._A3x, eps)
    }

    pub fn _C3f(&self, eps: f64, c: &mut [f64; GEODESIC_ORDER]) {
        let mut mult = 1.0;
        let mut o = 0;
        // Clippy wants us to turn this into `c.iter_mut().enumerate().take(geodesic_order + 1).skip(1)`
        // but benching (rust-1.75) shows that it would be slower.
        #[allow(clippy::needless_range_loop)]
        for l in 1..GEODESIC_ORDER {
            let m = GEODESIC_ORDER - l - 1;
            mult *= eps;
            c[l] = mult * geomath::polyval(m, &self._C3x[o..], eps);
            o += m + 1;
        }
    }

    pub fn _C4f(&self, eps: f64, c: &mut [f64; GEODESIC_ORDER]) {
        let mut mult = 1.0;
        let mut o = 0;
        // Clippy wants us to turn this into `c.iter_mut().enumerate().take(geodesic_order + 1).skip(1)`
        // but benching (rust-1.75) shows that it would be slower.
        #[allow(clippy::needless_range_loop)]
        for l in 0..GEODESIC_ORDER {
            let m = GEODESIC_ORDER - l - 1;
            c[l] = mult * geomath::polyval(m, &self._C4x[o..], eps);
            o += m + 1;
            mult *= eps;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn _Lengths(
        &self,
        eps: f64,
        sig12: f64,
        ssig1: f64,
        csig1: f64,
        dn1: f64,
        ssig2: f64,
        csig2: f64,
        dn2: f64,
        cbet1: f64,
        cbet2: f64,
        outmask: u64,
        C1a: &mut [f64; CARR_SIZE],
        C2a: &mut [f64; CARR_SIZE],
    ) -> (f64, f64, f64, f64, f64) {
        let outmask = outmask & OUT_MASK;
        let mut s12b = f64::NAN;
        let mut m12b = f64::NAN;
        let mut m0 = f64::NAN;
        let mut M12 = f64::NAN;
        let mut M21 = f64::NAN;

        let mut A1 = 0.0;
        let mut A2 = 0.0;
        let mut m0x = 0.0;
        let mut J12 = 0.0;

        if outmask & (DISTANCE | REDUCEDLENGTH | GEODESICSCALE) != 0 {
            A1 = geomath::_A1m1f(eps);
            geomath::_C1f(eps, C1a);
            if outmask & (REDUCEDLENGTH | GEODESICSCALE) != 0 {
                A2 = geomath::_A2m1f(eps);
                geomath::_C2f(eps, C2a);
                m0x = A1 - A2;
                A2 += 1.0;
            }
            A1 += 1.0;
        }
        if outmask & DISTANCE != 0 {
            let B1 = geomath::sin_cos_series(true, ssig2, csig2, C1a)
                - geomath::sin_cos_series(true, ssig1, csig1, C1a);
            s12b = A1 * (sig12 + B1);
            if outmask & (REDUCEDLENGTH | GEODESICSCALE) != 0 {
                let B2 = geomath::sin_cos_series(true, ssig2, csig2, C2a)
                    - geomath::sin_cos_series(true, ssig1, csig1, C2a);
                J12 = m0x * sig12 + (A1 * B1 - A2 * B2);
            }
        } else if outmask & (REDUCEDLENGTH | GEODESICSCALE) != 0 {
            for l in 1..=GEODESIC_ORDER {
                C2a[l] = A1 * C1a[l] - A2 * C2a[l];
            }
            J12 = m0x * sig12
                + (geomath::sin_cos_series(true, ssig2, csig2, C2a)
                    - geomath::sin_cos_series(true, ssig1, csig1, C2a));
        }
        if outmask & REDUCEDLENGTH != 0 {
            m0 = m0x;
            // J12 is wrong
            m12b = dn2 * (csig1 * ssig2) - dn1 * (ssig1 * csig2) - csig1 * csig2 * J12;
        }
        if outmask & GEODESICSCALE != 0 {
            let csig12 = csig1 * csig2 + ssig1 * ssig2;
            let t = self._ep2 * (cbet1 - cbet2) * (cbet1 + cbet2) / (dn1 + dn2);
            M12 = csig12 + (t * ssig2 - csig2 * J12) * ssig1 / dn1;
            M21 = csig12 - (t * ssig1 - csig1 * J12) * ssig2 / dn2;
        }
        (s12b, m12b, m0, M12, M21)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn _InverseStart(
        &self,
        sbet1: f64,
        cbet1: f64,
        dn1: f64,
        sbet2: f64,
        cbet2: f64,
        dn2: f64,
        lam12: f64,
        slam12: f64,
        clam12: f64,
        C1a: &mut [f64; CARR_SIZE],
        C2a: &mut [f64; CARR_SIZE],
    ) -> (f64, f64, f64, f64, f64, f64) {
        let mut sig12 = -1.0;
        let mut salp2 = f64::NAN;
        let mut calp2 = f64::NAN;
        let mut dnm = f64::NAN;

        let mut somg12: f64;
        let mut comg12: f64;

        let sbet12 = sbet2 * cbet1 - cbet2 * sbet1;
        let cbet12 = cbet2 * cbet1 + sbet2 * sbet1;

        let mut sbet12a = sbet2 * cbet1;
        sbet12a += cbet2 * sbet1;

        let shortline = cbet12 >= 0.0 && sbet12 < 0.5 && cbet2 * lam12 < 0.5;
        if shortline {
            let mut sbetm2 = geomath::sq(sbet1 + sbet2);
            sbetm2 /= sbetm2 + geomath::sq(cbet1 + cbet2);
            dnm = (1.0 + self._ep2 * sbetm2).sqrt();
            let omg12 = lam12 / (self._f1 * dnm);
            somg12 = omg12.sin();
            comg12 = omg12.cos();
        } else {
            somg12 = slam12;
            comg12 = clam12;
        }

        let mut salp1 = cbet2 * somg12;

        let mut calp1 = if comg12 >= 0.0 {
            sbet12 + cbet2 * sbet1 * geomath::sq(somg12) / (1.0 + comg12)
        } else {
            sbet12a - cbet2 * sbet1 * geomath::sq(somg12) / (1.0 - comg12)
        };

        let ssig12 = salp1.hypot(calp1);
        let csig12 = sbet1 * sbet2 + cbet1 * cbet2 * comg12;

        if shortline && ssig12 < self._etol2 {
            salp2 = cbet1 * somg12;
            calp2 = sbet12
                - cbet1
                    * sbet2
                    * (if comg12 >= 0.0 {
                        geomath::sq(somg12) / (1.0 + comg12)
                    } else {
                        1.0 - comg12
                    });
            geomath::norm(&mut salp2, &mut calp2);
            sig12 = ssig12.atan2(csig12);
        } else if self._n.abs() > 0.1
            || csig12 >= 0.0
            || ssig12 >= 6.0 * self._n.abs() * PI * geomath::sq(cbet1)
        {
        } else {
            let x: f64;
            let y: f64;
            let betscale: f64;
            let lamscale: f64;
            let lam12x = (-slam12).atan2(-clam12);
            if self.f >= 0.0 {
                let k2 = geomath::sq(sbet1) * self._ep2;
                let eps = k2 / (2.0 * (1.0 + (1.0 + k2).sqrt()) + k2);
                lamscale = self.f * cbet1 * self._A3f(eps) * PI;
                betscale = lamscale * cbet1;
                x = lam12x / lamscale;
                y = sbet12a / betscale;
            } else {
                let cbet12a = cbet2 * cbet1 - sbet2 * sbet1;
                let bet12a = sbet12a.atan2(cbet12a);
                let (_, m12b, m0, _, _) = self._Lengths(
                    self._n,
                    PI + bet12a,
                    sbet1,
                    -cbet1,
                    dn1,
                    sbet2,
                    cbet2,
                    dn2,
                    cbet1,
                    cbet2,
                    REDUCEDLENGTH,
                    C1a,
                    C2a,
                );
                x = -1.0 + m12b / (cbet1 * cbet2 * m0 * PI);
                betscale = if x < -0.01 {
                    sbet12a / x
                } else {
                    -self.f * geomath::sq(cbet1) * PI
                };
                lamscale = betscale / cbet1;
                y = lam12x / lamscale;
            }
            if y > -self.tol1_ && x > -1.0 - self.xthresh_ {
                if self.f >= 0.0 {
                    salp1 = (-x).min(1.0);
                    calp1 = -(1.0 - geomath::sq(salp1)).sqrt()
                } else {
                    calp1 = x.max(if x > -self.tol1_ { 0.0 } else { -1.0 });
                    salp1 = (1.0 - geomath::sq(calp1)).sqrt();
                }
            } else {
                let k = geomath::astroid(x, y);
                let omg12a = lamscale
                    * if self.f >= 0.0 {
                        -x * k / (1.0 + k)
                    } else {
                        -y * (1.0 + k) / k
                    };
                somg12 = omg12a.sin();
                comg12 = -(omg12a.cos());
                salp1 = cbet2 * somg12;
                calp1 = sbet12a - cbet2 * sbet1 * geomath::sq(somg12) / (1.0 - comg12);
            }
        }

        if salp1 > 0.0 || salp1.is_nan() {
            geomath::norm(&mut salp1, &mut calp1);
        } else {
            salp1 = 1.0;
            calp1 = 0.0;
        };
        (sig12, salp1, calp1, salp2, calp2, dnm)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn _Lambda12(
        &self,
        sbet1: f64,
        cbet1: f64,
        dn1: f64,
        sbet2: f64,
        cbet2: f64,
        dn2: f64,
        salp1: f64,
        mut calp1: f64,
        slam120: f64,
        clam120: f64,
        diffp: bool,
        C1a: &mut [f64; CARR_SIZE],
        C2a: &mut [f64; CARR_SIZE],
        C3a: &mut [f64; GEODESIC_ORDER],
    ) -> (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) {
        if sbet1 == 0.0 && calp1 == 0.0 {
            calp1 = -self.tiny_;
        }
        let salp0 = salp1 * cbet1;
        let calp0 = calp1.hypot(salp1 * sbet1);

        let mut ssig1 = sbet1;
        let somg1 = salp0 * sbet1;
        let mut csig1 = calp1 * cbet1;
        let comg1 = calp1 * cbet1;
        geomath::norm(&mut ssig1, &mut csig1);

        let salp2 = if cbet2 != cbet1 { salp0 / cbet2 } else { salp1 };
        let calp2 = if cbet2 != cbet1 || sbet2.abs() != -sbet1 {
            (geomath::sq(calp1 * cbet1)
                + if cbet1 < -sbet1 {
                    (cbet2 - cbet1) * (cbet1 + cbet2)
                } else {
                    (sbet1 - sbet2) * (sbet1 + sbet2)
                })
            .sqrt()
                / cbet2
        } else {
            calp1.abs()
        };
        let mut ssig2 = sbet2;
        let somg2 = salp0 * sbet2;
        let mut csig2 = calp2 * cbet2;
        let comg2 = calp2 * cbet2;
        geomath::norm(&mut ssig2, &mut csig2);

        let sig12 = ((csig1 * ssig2 - ssig1 * csig2).max(0.0)).atan2(csig1 * csig2 + ssig1 * ssig2);
        let somg12 = (comg1 * somg2 - somg1 * comg2).max(0.0);
        let comg12 = comg1 * comg2 + somg1 * somg2;
        let eta = (somg12 * clam120 - comg12 * slam120).atan2(comg12 * clam120 + somg12 * slam120);

        let k2 = geomath::sq(calp0) * self._ep2;
        let eps = k2 / (2.0 * (1.0 + (1.0 + k2).sqrt()) + k2);
        self._C3f(eps, C3a);
        let B312 = geomath::sin_cos_series(true, ssig2, csig2, C3a)
            - geomath::sin_cos_series(true, ssig1, csig1, C3a);
        let domg12 = -self.f * self._A3f(eps) * salp0 * (sig12 + B312);
        let lam12 = eta + domg12;

        let mut dlam12: f64;
        if diffp {
            if calp2 == 0.0 {
                dlam12 = -2.0 * self._f1 * dn1 / sbet1;
            } else {
                let res = self._Lengths(
                    eps,
                    sig12,
                    ssig1,
                    csig1,
                    dn1,
                    ssig2,
                    csig2,
                    dn2,
                    cbet1,
                    cbet2,
                    REDUCEDLENGTH,
                    C1a,
                    C2a,
                );
                dlam12 = res.1;
                dlam12 *= self._f1 / (calp2 * cbet2);
            }
        } else {
            dlam12 = f64::NAN;
        }
        (
            lam12, salp2, calp2, sig12, ssig1, csig1, ssig2, csig2, eps, domg12, dlam12,
        )
    }

    // returns (a12, s12, salp1, calp1, salp2, calp2, m12, M12, M21, S12)
    pub fn _gen_inverse(
        &self,
        lat1: f64,
        lon1: f64,
        lat2: f64,
        lon2: f64,
        outmask: u64,
    ) -> (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) {
        let mut lat1 = lat1;
        let mut lat2 = lat2;
        let mut a12 = f64::NAN;
        let mut s12 = f64::NAN;
        let mut m12 = f64::NAN;
        let mut M12 = f64::NAN;
        let mut M21 = f64::NAN;
        let mut S12 = f64::NAN;
        let outmask = outmask & OUT_MASK;

        let (mut lon12, mut lon12s) = geomath::ang_diff(lon1, lon2);
        let mut lonsign = if lon12 >= 0.0 { 1.0 } else { -1.0 };

        lon12 = lonsign * geomath::ang_round(lon12);
        lon12s = geomath::ang_round((180.0 - lon12) - lonsign * lon12s);
        let lam12 = lon12.to_radians();
        let slam12: f64;
        let mut clam12: f64;
        if lon12 > 90.0 {
            let res = geomath::sincosd(lon12s);
            slam12 = res.0;
            clam12 = res.1;
            clam12 = -clam12;
        } else {
            let res = geomath::sincosd(lon12);
            slam12 = res.0;
            clam12 = res.1;
        };
        lat1 = geomath::ang_round(geomath::lat_fix(lat1));
        lat2 = geomath::ang_round(geomath::lat_fix(lat2));

        let swapp = if lat1.abs() < lat2.abs() { -1.0 } else { 1.0 };
        if swapp < 0.0 {
            lonsign *= -1.0;
            std::mem::swap(&mut lat2, &mut lat1);
        }
        let latsign = if lat1 < 0.0 { 1.0 } else { -1.0 };
        lat1 *= latsign;
        lat2 *= latsign;

        let (mut sbet1, mut cbet1) = geomath::sincosd(lat1);
        sbet1 *= self._f1;

        geomath::norm(&mut sbet1, &mut cbet1);
        cbet1 = cbet1.max(self.tiny_);

        let (mut sbet2, mut cbet2) = geomath::sincosd(lat2);
        sbet2 *= self._f1;

        geomath::norm(&mut sbet2, &mut cbet2);
        cbet2 = cbet2.max(self.tiny_);

        if cbet1 < -sbet1 {
            if cbet2 == cbet1 {
                sbet2 = if sbet2 < 0.0 { sbet1 } else { -sbet1 };
            }
        } else if sbet2.abs() == -sbet1 {
            cbet2 = cbet1;
        }

        let dn1 = (1.0 + self._ep2 * geomath::sq(sbet1)).sqrt();
        let dn2 = (1.0 + self._ep2 * geomath::sq(sbet2)).sqrt();

        let mut C1a: [f64; CARR_SIZE] = [0.0; CARR_SIZE];
        let mut C2a: [f64; CARR_SIZE] = [0.0; CARR_SIZE];
        let mut C3a: [f64; GEODESIC_ORDER] = [0.0; GEODESIC_ORDER];

        let mut meridian = lat1 == -90.0 || slam12 == 0.0;
        let mut calp1 = 0.0;
        let mut salp1 = 0.0;
        let mut calp2 = 0.0;
        let mut salp2 = 0.0;
        let mut ssig1 = 0.0;
        let mut csig1 = 0.0;
        let mut ssig2 = 0.0;
        let mut csig2 = 0.0;
        let mut sig12: f64;
        let mut s12x = 0.0;
        let mut m12x = 0.0;

        if meridian {
            calp1 = clam12;
            salp1 = slam12;
            calp2 = 1.0;
            salp2 = 0.0;

            ssig1 = sbet1;
            csig1 = calp1 * cbet1;
            ssig2 = sbet2;
            csig2 = calp2 * cbet2;

            sig12 = ((csig1 * ssig2 - ssig1 * csig2).max(0.0)).atan2(csig1 * csig2 + ssig1 * ssig2);
            let res = self._Lengths(
                self._n,
                sig12,
                ssig1,
                csig1,
                dn1,
                ssig2,
                csig2,
                dn2,
                cbet1,
                cbet2,
                outmask | DISTANCE | REDUCEDLENGTH,
                &mut C1a,
                &mut C2a,
            );
            s12x = res.0;
            m12x = res.1;
            M12 = res.3;
            M21 = res.4;

            if sig12 < 1.0 || m12x >= 0.0 {
                if sig12 < 3.0 * self.tiny_ {
                    sig12 = 0.0;
                    m12x = 0.0;
                    s12x = 0.0;
                }
                m12x *= self._b;
                s12x *= self._b;
                a12 = sig12.to_degrees();
            } else {
                meridian = false;
            }
        }

        let mut somg12 = 2.0;
        let mut comg12 = 0.0;
        let mut omg12 = 0.0;
        let dnm: f64;
        let mut eps = 0.0;
        if !meridian && sbet1 == 0.0 && (self.f <= 0.0 || lon12s >= self.f * 180.0) {
            calp1 = 0.0;
            calp2 = 0.0;
            salp1 = 1.0;
            salp2 = 1.0;

            s12x = self.a * lam12;
            sig12 = lam12 / self._f1;
            omg12 = lam12 / self._f1;
            m12x = self._b * sig12.sin();
            if outmask & GEODESICSCALE != 0 {
                M12 = sig12.cos();
                M21 = sig12.cos();
            }
            a12 = lon12 / self._f1;
        } else if !meridian {
            let res = self._InverseStart(
                sbet1, cbet1, dn1, sbet2, cbet2, dn2, lam12, slam12, clam12, &mut C1a, &mut C2a,
            );
            sig12 = res.0;
            salp1 = res.1;
            calp1 = res.2;
            salp2 = res.3;
            calp2 = res.4;
            dnm = res.5;

            if sig12 >= 0.0 {
                s12x = sig12 * self._b * dnm;
                m12x = geomath::sq(dnm) * self._b * (sig12 / dnm).sin();
                if outmask & GEODESICSCALE != 0 {
                    M12 = (sig12 / dnm).cos();
                    M21 = (sig12 / dnm).cos();
                }
                a12 = sig12.to_degrees();
                omg12 = lam12 / (self._f1 * dnm);
            } else {
                let mut tripn = false;
                let mut tripb = false;
                let mut salp1a = self.tiny_;
                let mut calp1a = 1.0;
                let mut salp1b = self.tiny_;
                let mut calp1b = -1.0;
                let mut domg12 = 0.0;
                for numit in 0..self.maxit2_ {
                    let res = self._Lambda12(
                        sbet1,
                        cbet1,
                        dn1,
                        sbet2,
                        cbet2,
                        dn2,
                        salp1,
                        calp1,
                        slam12,
                        clam12,
                        numit < self.maxit1_,
                        &mut C1a,
                        &mut C2a,
                        &mut C3a,
                    );
                    let v = res.0;
                    salp2 = res.1;
                    calp2 = res.2;
                    sig12 = res.3;
                    ssig1 = res.4;
                    csig1 = res.5;
                    ssig2 = res.6;
                    csig2 = res.7;
                    eps = res.8;
                    domg12 = res.9;
                    let dv = res.10;

                    if tripb
                        || v.abs() < if tripn { 8.0 } else { 1.0 } * self.tol0_
                        || v.abs().is_nan()
                    {
                        break;
                    };
                    if v > 0.0 && (numit > self.maxit1_ || calp1 / salp1 > calp1b / salp1b) {
                        salp1b = salp1;
                        calp1b = calp1;
                    } else if v < 0.0 && (numit > self.maxit1_ || calp1 / salp1 < calp1a / salp1a) {
                        salp1a = salp1;
                        calp1a = calp1;
                    }
                    if numit < self.maxit1_ && dv > 0.0 {
                        let dalp1 = -v / dv;
                        let sdalp1 = dalp1.sin();
                        let cdalp1 = dalp1.cos();
                        let nsalp1 = salp1 * cdalp1 + calp1 * sdalp1;
                        if nsalp1 > 0.0 && dalp1.abs() < PI {
                            calp1 = calp1 * cdalp1 - salp1 * sdalp1;
                            salp1 = nsalp1;
                            geomath::norm(&mut salp1, &mut calp1);
                            tripn = v.abs() <= 16.0 * self.tol0_;
                            continue;
                        }
                    }

                    salp1 = (salp1a + salp1b) / 2.0;
                    calp1 = (calp1a + calp1b) / 2.0;
                    geomath::norm(&mut salp1, &mut calp1);
                    tripn = false;
                    tripb = (salp1a - salp1).abs() + (calp1a - calp1) < self.tolb_
                        || (salp1 - salp1b).abs() + (calp1 - calp1b) < self.tolb_;
                }
                let lengthmask = outmask
                    | if outmask & (REDUCEDLENGTH | GEODESICSCALE) != 0 {
                        DISTANCE
                    } else {
                        EMPTY
                    };
                let res = self._Lengths(
                    eps, sig12, ssig1, csig1, dn1, ssig2, csig2, dn2, cbet1, cbet2, lengthmask,
                    &mut C1a, &mut C2a,
                );
                s12x = res.0;
                m12x = res.1;
                M12 = res.3;
                M21 = res.4;

                m12x *= self._b;
                s12x *= self._b;
                a12 = sig12.to_degrees();
                if outmask & AREA != 0 {
                    let sdomg12 = domg12.sin();
                    let cdomg12 = domg12.cos();
                    somg12 = slam12 * cdomg12 - clam12 * sdomg12;
                    comg12 = clam12 * cdomg12 + slam12 * sdomg12;
                }
            }
        }
        if outmask & DISTANCE != 0 {
            s12 = 0.0 + s12x;
        }
        if outmask & REDUCEDLENGTH != 0 {
            m12 = 0.0 + m12x;
        }
        if outmask & AREA != 0 {
            let salp0 = salp1 * cbet1;
            let calp0 = calp1.hypot(salp1 * sbet1);
            if calp0 != 0.0 && salp0 != 0.0 {
                ssig1 = sbet1;
                csig1 = calp1 * cbet1;
                ssig2 = sbet2;
                csig2 = calp2 * cbet2;
                let k2 = geomath::sq(calp0) * self._ep2;
                eps = k2 / (2.0 * (1.0 + (1.0 + k2).sqrt()) + k2);
                let A4 = geomath::sq(self.a) * calp0 * salp0 * self._e2;
                geomath::norm(&mut ssig1, &mut csig1);
                geomath::norm(&mut ssig2, &mut csig2);
                let mut C4a: [f64; GEODESIC_ORDER] = [0.0; GEODESIC_ORDER];
                self._C4f(eps, &mut C4a);
                let B41 = geomath::sin_cos_series(false, ssig1, csig1, &C4a);
                let B42 = geomath::sin_cos_series(false, ssig2, csig2, &C4a);
                S12 = A4 * (B42 - B41);
            } else {
                S12 = 0.0;
            }

            if !meridian && somg12 > 1.0 {
                somg12 = omg12.sin();
                comg12 = omg12.cos();
            }

            // We're diverging from Karney's implementation here
            // which uses the hardcoded constant: -0.7071 for FRAC_1_SQRT_2
            let alp12: f64;
            if !meridian && comg12 > -FRAC_1_SQRT_2 && sbet2 - sbet1 < 1.75 {
                let domg12 = 1.0 + comg12;
                let dbet1 = 1.0 + cbet1;
                let dbet2 = 1.0 + cbet2;
                alp12 = 2.0
                    * (somg12 * (sbet1 * dbet2 + sbet2 * dbet1))
                        .atan2(domg12 * (sbet1 * sbet2 + dbet1 * dbet2));
            } else {
                let mut salp12 = salp2 * calp1 - calp2 * salp1;
                let mut calp12 = calp2 * calp1 + salp2 * salp1;

                if salp12 == 0.0 && calp12 < 0.0 {
                    salp12 = self.tiny_ * calp1;
                    calp12 = -1.0;
                }
                alp12 = salp12.atan2(calp12);
            }
            S12 += self._c2 * alp12;
            S12 *= swapp * lonsign * latsign;
            S12 += 0.0;
        }

        if swapp < 0.0 {
            std::mem::swap(&mut salp2, &mut salp1);

            std::mem::swap(&mut calp2, &mut calp1);

            if outmask & GEODESICSCALE != 0 {
                std::mem::swap(&mut M21, &mut M12);
            }
        }
        salp1 *= swapp * lonsign;
        calp1 *= swapp * latsign;
        salp2 *= swapp * lonsign;
        calp2 *= swapp * latsign;
        (a12, s12, salp1, calp1, salp2, calp2, m12, M12, M21, S12)
    }

    pub fn inverse_distance(&self, lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        let (_a12, s12, _salp1, _calp1, _salp2, _calp2, _m12, _M12, _M21, _S12) =
            self._gen_inverse(lat1, lon1, lat2, lon2, DISTANCE);
        s12
    }
}
