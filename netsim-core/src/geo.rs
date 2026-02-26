use crate::measure::Latency;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum GeoError {
    #[error(
        "latitude out of range [{min}, {max}] in e4 units: {value}",
        min = Latitude::MIN_E4,
        max = Latitude::MAX_E4
    )]
    InvalidLatitude { value: i32 },
    #[error(
        "longitude out of range [{min}, {max}] in e4 units: {value}",
        min = Longitude::MIN_E4,
        max = Longitude::MAX_E4
    )]
    InvalidLongitude { value: i32 },
    #[error("fiber speed ratio must be finite and within [0.01, 1.0], got {value}")]
    InvalidFiberSpeedRatio { value: f64 },
    #[error("vincenty inverse formula did not converge")]
    NonConvergent,
    #[error("geo computation produced a non-finite value")]
    NonFiniteComputation,
}

/// Latitude in e4 fixed-point format (`degrees * 10_000`).
///
/// `48.8534°` is represented as `488_534`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Latitude(i32);

impl Latitude {
    pub const MIN_E4: i32 = -90_0000;
    pub const MAX_E4: i32 = 90_0000;

    pub fn try_from_e4(value: i32) -> Result<Self, GeoError> {
        if !(Self::MIN_E4..=Self::MAX_E4).contains(&value) {
            return Err(GeoError::InvalidLatitude { value });
        }

        Ok(Self(value))
    }

    pub fn from_degrees(value: f64) -> Result<Self, GeoError> {
        if !value.is_finite() {
            return Err(GeoError::NonFiniteComputation);
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

    pub fn try_from_e4(value: i32) -> Result<Self, GeoError> {
        if !(Self::MIN_E4..=Self::MAX_E4).contains(&value) {
            return Err(GeoError::InvalidLongitude { value });
        }

        Ok(Self(value))
    }

    pub fn from_degrees(value: f64) -> Result<Self, GeoError> {
        if !value.is_finite() {
            return Err(GeoError::NonFiniteComputation);
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

    pub fn try_from_e4(latitude: i32, longitude: i32) -> Result<Self, GeoError> {
        Ok(Self::new(
            Latitude::try_from_e4(latitude)?,
            Longitude::try_from_e4(longitude)?,
        ))
    }

    pub fn from_degrees(latitude: f64, longitude: f64) -> Result<Self, GeoError> {
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
fn distance_between(point1: Location, point2: Location) -> Result<f64, GeoError> {
    let distance = VincentyInverse::default()
        .calculate(point1, point2, Spheroid::earth())
        .ok_or(GeoError::NonConvergent)?;

    if !distance.is_finite() {
        return Err(GeoError::NonFiniteComputation);
    }

    Ok(distance)
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
/// Known limitation:
/// this iterative method can fail to converge for nearly antipodal point pairs
/// (including exact antipodes). In this module, hitting the iteration limit
/// maps to `GeoError::NonConvergent`.
///
/// For an explicit reproduction, see the `antipodal_points_can_fail_to_converge`
/// test.
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
    /// The computation may fail to converge for some point pairs (notably
    /// nearly antipodal pairs), in which case `None` is returned.
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

pub fn latency_between_locations(
    p1: Location,
    p2: Location,
    sol_fo: f64,
) -> Result<Latency, GeoError> {
    const SPEED_OF_LIGHT: f64 = 299_792_458.0; // meter per second
    const SPEED_OF_FIBER: f64 = SPEED_OF_LIGHT * 0.69; // light travels 31% slower in fiber optics

    if !sol_fo.is_finite() || !(0.01..=1.0).contains(&sol_fo) {
        return Err(GeoError::InvalidFiberSpeedRatio { value: sol_fo });
    }

    let distance = distance_between(p1, p2)?;
    let latency_us = distance / (SPEED_OF_FIBER * sol_fo) * 1_000_000.0;

    if !latency_us.is_finite() || latency_us < 0.0 || latency_us > (u64::MAX as f64) {
        return Err(GeoError::NonFiniteComputation);
    }

    // Round to nearest microsecond to match Latency precision without systematic floor bias.
    let latency_us = latency_us.round() as u64;

    Ok(Latency::new(std::time::Duration::from_micros(latency_us)))
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

        assert_eq!(latency.to_string(), "122ms512µs");
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
        assert_eq!(
            Location::try_from_e4(91_0000, 0).unwrap_err(),
            GeoError::InvalidLatitude { value: 91_0000 }
        );
    }

    #[test]
    fn rejects_invalid_longitude() {
        assert_eq!(
            Location::try_from_e4(0, 181_0000).unwrap_err(),
            GeoError::InvalidLongitude { value: 181_0000 }
        );
    }

    #[test]
    fn rejects_invalid_fiber_speed_ratio() {
        assert_eq!(
            latency_between_locations(p1(), p2(), 0.0).unwrap_err(),
            GeoError::InvalidFiberSpeedRatio { value: 0.0 }
        );
    }

    #[test]
    fn rejects_non_finite_coordinate_degrees() {
        assert_eq!(
            Location::from_degrees(f64::NAN, 0.0).unwrap_err(),
            GeoError::NonFiniteComputation
        );
    }

    #[test]
    fn short_distance_keeps_microsecond_precision() {
        let p1 = Location::try_from_e4(0, 0).unwrap();
        let p2 = Location::try_from_e4(0_0100, 0).unwrap();

        let latency = latency_between_locations(p1, p2, 1.0).unwrap();
        let duration = latency.into_duration();
        assert!(duration > std::time::Duration::ZERO);
        assert!(duration < std::time::Duration::from_millis(1));
    }

    #[test]
    fn antipodal_points_can_fail_to_converge() {
        let p1 = Location::try_from_e4(0, 0).unwrap();
        let p2 = Location::try_from_e4(0, 180_0000).unwrap();

        assert_eq!(
            latency_between_locations(p1, p2, 1.0).unwrap_err(),
            GeoError::NonConvergent
        );
    }
}
