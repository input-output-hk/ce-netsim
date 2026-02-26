use crate::measure::Latency;
use anyhow::{anyhow, ensure};

/// Latitude in e4 fixed-point format (`degrees * 10_000`).
///
/// `48.8534°` is represented as `488_534`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Latitude(i32);

impl Latitude {
    pub const MIN_E4: i32 = -90_0000;
    pub const MAX_E4: i32 = 90_0000;

    pub fn try_from_e4(value: i32) -> anyhow::Result<Self> {
        ensure!(
            (Self::MIN_E4..=Self::MAX_E4).contains(&value),
            "latitude out of range [{}, {}] in e4 units: {}",
            Self::MIN_E4,
            Self::MAX_E4,
            value
        );
        Ok(Self(value))
    }

    pub fn from_degrees(value: f64) -> anyhow::Result<Self> {
        if !value.is_finite() {
            return Err(anyhow!("latitude must be finite"));
        }

        let scaled = (value * 10_000.0).round();
        Self::try_from_e4(scaled as i32)
    }

    pub const fn as_e4(self) -> i32 {
        self.0
    }

    fn to_radians(self) -> f64 {
        (self.0 as f64 / 10_000.0).to_radians()
    }
}

/// Longitude in e4 fixed-point format (`degrees * 10_000`).
///
/// `-122.4194°` is represented as `-1_224_194`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Longitude(i32);

impl Longitude {
    pub const MIN_E4: i32 = -180_0000;
    pub const MAX_E4: i32 = 180_0000;

    pub fn try_from_e4(value: i32) -> anyhow::Result<Self> {
        ensure!(
            (Self::MIN_E4..=Self::MAX_E4).contains(&value),
            "longitude out of range [{}, {}] in e4 units: {}",
            Self::MIN_E4,
            Self::MAX_E4,
            value
        );
        Ok(Self(value))
    }

    pub fn from_degrees(value: f64) -> anyhow::Result<Self> {
        if !value.is_finite() {
            return Err(anyhow!("longitude must be finite"));
        }

        let scaled = (value * 10_000.0).round();
        Self::try_from_e4(scaled as i32)
    }

    pub const fn as_e4(self) -> i32 {
        self.0
    }

    fn to_radians(self) -> f64 {
        (self.0 as f64 / 10_000.0).to_radians()
    }
}

/// Location using validated latitude and longitude coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Location {
    pub latitude: Latitude,
    pub longitude: Longitude,
}

impl Location {
    pub const fn new(latitude: Latitude, longitude: Longitude) -> Self {
        Self {
            latitude,
            longitude,
        }
    }

    pub fn try_from_e4(latitude: i32, longitude: i32) -> anyhow::Result<Self> {
        Ok(Self::new(
            Latitude::try_from_e4(latitude)?,
            Longitude::try_from_e4(longitude)?,
        ))
    }

    pub fn from_degrees(latitude: f64, longitude: f64) -> anyhow::Result<Self> {
        Ok(Self::new(
            Latitude::from_degrees(latitude)?,
            Longitude::from_degrees(longitude)?,
        ))
    }

    fn to_radians(self) -> (f64, f64) {
        (self.latitude.to_radians(), self.longitude.to_radians())
    }
}

// return the distance in meter between point1 and point2
fn distance_between(point1: Location, point2: Location) -> Option<f64> {
    VincentyInverse::default().calculate(point1, point2, Spheroid::earth())
}

/// Spheroid parameter
///
/// for earth, use `Spheroid::earth()`
///
/// using WGS-84 geocentric datum parameters
///
/// It should hold that beta = (1.0 - inv_flattening) * alpha;
struct Spheroid {
    /// Semi Major Axis in meter / Radius at equator
    alpha: f64,
    /// Semi Minor axis in meter / Radius at pole
    beta: f64,
    /// inverse flattening 1/f
    inv_flattening: f64,
}

impl Spheroid {
    const fn new(alpha: f64, beta: f64, inv_flattening: f64) -> Self {
        Self {
            alpha,
            beta,
            inv_flattening,
        }
    }

    const fn earth() -> Self {
        Self::new(
            6378137.0,
            6356752.314245,
            0.00335281066, // 1.0 / 298.257223563,
        )
    }
}

/// Vincenty inverse formula, parametrized with the number of maximum iterations
/// for the algorithm
///
/// [Wikipedia Vincenty formulae](https://en.wikipedia.org/wiki/Vincenty%27s_formulae)
#[derive(Clone, Debug)]
struct VincentyInverse {
    nb_iter: usize,
}

impl Default for VincentyInverse {
    fn default() -> Self {
        Self { nb_iter: 50 }
    }
}

trait SpheroidDistanceAlgorithm {
    /// Try to calculate the distance between two points on a spheroid, using a formula.
    ///
    /// Algorithm can fails to compute, and may return None
    fn calculate(&self, point1: Location, point2: Location, spheroid: Spheroid) -> Option<f64>;
}

impl SpheroidDistanceAlgorithm for VincentyInverse {
    fn calculate(&self, point1: Location, point2: Location, spheroid: Spheroid) -> Option<f64> {
        let a = spheroid.alpha;
        let b = spheroid.beta;
        let f = spheroid.inv_flattening;

        let p1 = point1.to_radians();
        let p2 = point2.to_radians();

        let difference_longitudes = p2.1 - p1.1;

        // u = 'reduced latitude'
        let (tan_u1, tan_u2) = ((1.0 - f) * p1.0.tan(), (1.0 - f) * p2.0.tan());
        let (cos_u1, cos_u2) = (
            1.0 / (1.0 + tan_u1 * tan_u1).sqrt(),
            1.0 / (1.0 + tan_u2 * tan_u2).sqrt(),
        );
        let (sin_u1, sin_u2) = (tan_u1 * cos_u1, tan_u2 * cos_u2);

        let mut lambda = difference_longitudes;
        let mut iter_limit = self.nb_iter;

        let mut cos_sq_alpha = 0.0;
        let mut sin_sigma = 0.0;
        let mut cos_sigma = 0.0;
        let mut cos2_sigma_m = 0.0;
        let mut sigma = 0.0;

        while iter_limit > 0 {
            let sin_lambda = lambda.sin();
            let cos_lambda = lambda.cos();
            let sin_sq_sigma = (cos_u2 * sin_lambda) * (cos_u2 * sin_lambda)
                + (cos_u1 * sin_u2 - sin_u1 * cos_u2 * cos_lambda)
                    * (cos_u1 * sin_u2 - sin_u1 * cos_u2 * cos_lambda);

            // Points coincide
            if sin_sq_sigma == 0.0 {
                break;
            }

            sin_sigma = sin_sq_sigma.sqrt();
            cos_sigma = sin_u1 * sin_u2 + cos_u1 * cos_u2 * cos_lambda;
            sigma = sin_sigma.atan2(cos_sigma);
            let sin_alpha = cos_u1 * cos_u2 * sin_lambda / sin_sigma;
            cos_sq_alpha = 1.0 - sin_alpha * sin_alpha;
            cos2_sigma_m = if cos_sq_alpha != 0.0 {
                cos_sigma - 2.0 * sin_u1 * sin_u2 / cos_sq_alpha
            } else {
                0.0
            };
            let c = f / 16.0 * cos_sq_alpha * (4.0 + f * (4.0 - 3.0 * cos_sq_alpha));
            let lambda_prime = lambda;
            lambda = difference_longitudes
                + (1.0 - c)
                    * f
                    * sin_alpha
                    * (sigma
                        + c * sin_sigma
                            * (cos2_sigma_m
                                + c * cos_sigma * (-1.0 + 2.0 * cos2_sigma_m * cos2_sigma_m)));

            // leave the loop if it has converged
            if (lambda - lambda_prime).abs() <= 1e-12 {
                break;
            }
            iter_limit -= 1;
        }

        if iter_limit == 0 {
            return None;
        }

        let u_sq = cos_sq_alpha * (a * a - b * b) / (b * b);
        let cap_a =
            1.0 + u_sq / 16384.0 * (4096.0 + u_sq * (-768.0 + u_sq * (320.0 - 175.0 * u_sq)));
        let cap_b = u_sq / 1024.0 * (256.0 + u_sq * (-128.0 + u_sq * (74.0 - 47.0 * u_sq)));

        let delta_sigma = cap_b
            * sin_sigma
            * (cos2_sigma_m
                + cap_b / 4.0
                    * (cos_sigma * (-1.0 + 2.0 * cos2_sigma_m * cos2_sigma_m)
                        - cap_b / 6.0
                            * cos2_sigma_m
                            * (-3.0 + 4.0 * sin_sigma * sin_sigma)
                            * (-3.0 + 4.0 * cos2_sigma_m * cos2_sigma_m)));
        let s = b * cap_a * (sigma - delta_sigma);

        Some(s)
    }
}

pub fn latency_between_locations(p1: Location, p2: Location, sol_fo: f64) -> Option<Latency> {
    const SPEED_OF_LIGHT: f64 = 299_792_458.0; // meter per second
    const SPEED_OF_FIBER: f64 = SPEED_OF_LIGHT * 0.69; // light travels 31% slower in fiber optics

    let sol_fo = sol_fo.clamp(0.01, 1.0);

    let distance = distance_between(p1, p2);
    distance.map(|d| {
        Latency::new(std::time::Duration::from_millis(
            (d / (SPEED_OF_FIBER * sol_fo) * 1000.0) as u64,
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOL_FO: f64 = 0.5;

    fn p1() -> Location {
        // 48.853415254543435, 2.3487911014845038
        Location::try_from_e4(48_8534, 2_3487).unwrap()
    }

    fn p2() -> Location {
        // -49.35231574277824, 70.2150600748867
        Location::try_from_e4(-49_3523, 70_2150).unwrap()
    }

    #[test]
    fn latency_between() {
        let latency = latency_between_locations(p1(), p2(), SOL_FO).unwrap();

        assert_eq!(latency.to_string(), "122ms");
    }

    #[test]
    fn latency_between_self() {
        let p1 = p1();
        let latency = latency_between_locations(p1, p1, SOL_FO).unwrap();

        assert_eq!(latency.to_string(), "0ms");
    }

    #[test]
    fn latency_between_no_enough_iter() {
        let v = VincentyInverse { nb_iter: 0 };

        assert!(v.calculate(p1(), p2(), Spheroid::earth()).is_none());
    }

    #[test]
    fn accepts_western_longitude() {
        assert!(Location::try_from_e4(37_7749, -122_4194).is_ok());
    }

    #[test]
    fn rejects_invalid_latitude() {
        assert!(Location::try_from_e4(91_0000, 0).is_err());
    }

    #[test]
    fn rejects_invalid_longitude() {
        assert!(Location::try_from_e4(0, 181_0000).is_err());
    }
}
