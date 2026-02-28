pub(crate) mod geodesic;
pub(crate) mod geomath;

use self::geodesic::Geodesic;

use crate::measure::Latency;
use std::{fmt, str::FromStr, sync::OnceLock};
use thiserror::Error;

/// Error returned when parsing a geo type ([`Latitude`], [`Longitude`],
/// [`Location`], or [`PathEfficiency`]) from a string fails.
#[derive(Debug, Error)]
pub enum GeoParseError {
    #[error("Failed to parse Latitude: {0}")]
    Latitude(String),
    #[error("Failed to parse Longitude: {0}")]
    Longitude(String),
    #[error("Failed to parse Location: expected format `<latitude>, <longitude>`")]
    LocationMissingComponent,
    #[error("Failed to parse Location: expected a single comma separator")]
    LocationExtraSeparator,
    #[error("Failed to parse Location {0}: {1}")]
    LocationComponent(&'static str, Box<GeoParseError>),
    #[error("Failed to parse PathEfficiency: {0}")]
    PathEfficiency(String),
}

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
    #[error("path efficiency must be finite and within (0.0, 1.0], got {value}")]
    InvalidPathEfficiency { value: f64 },
    #[error("vincenty inverse formula did not converge")]
    NonConvergent,
    #[error("geo computation produced a non-finite value")]
    NonFiniteComputation,
}

/// Latitude in e4 fixed-point format (`degrees * 10_000`).
///
/// `48.8534°` is represented as `488_534`.
///
/// # Parsing and display
///
/// ```
/// use netsim_core::geo::Latitude;
///
/// let latitude: Latitude = "48.8534".parse().unwrap();
/// assert_eq!(latitude.to_string(), "48.8534º");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Latitude(i32);

impl Latitude {
    pub const MIN_E4: i32 = -90_0000;
    pub const MAX_E4: i32 = 90_0000;

    /// Creates a latitude from e4 fixed-point units.
    ///
    /// Valid range: `[-90_0000, 90_0000]`.
    pub fn try_from_e4(value: i32) -> Result<Self, GeoError> {
        if !(Self::MIN_E4..=Self::MAX_E4).contains(&value) {
            return Err(GeoError::InvalidLatitude { value });
        }

        Ok(Self(value))
    }

    /// Creates a latitude from decimal degrees.
    ///
    /// Values are rounded to the nearest e4 fixed-point unit.
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

    /// Returns the latitude as decimal degrees.
    pub fn as_degrees(self) -> f64 {
        self.0 as f64 / 10_000.0
    }

    fn to_degrees(self) -> f64 {
        self.as_degrees()
    }

    fn to_radians(self) -> f64 {
        self.as_degrees().to_radians()
    }
}

impl fmt::Display for Latitude {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}{DEGREE_SUFFIX}", self.to_degrees())
    }
}

impl FromStr for Latitude {
    type Err = GeoParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let degrees = parse_coordinate_degrees(s).map_err(GeoParseError::Latitude)?;
        Self::from_degrees(degrees).map_err(|error| GeoParseError::Latitude(error.to_string()))
    }
}

/// Longitude in e4 fixed-point format (`degrees * 10_000`).
///
/// `-122.4194°` is represented as `-1_224_194`.
///
/// # Eastern and western examples
///
/// ```
/// use netsim_core::geo::Longitude;
///
/// let paris_east: Longitude = "2.3522".parse().unwrap();
/// let san_francisco_west: Longitude = "-122.4194".parse().unwrap();
///
/// assert_eq!(paris_east.as_e4(), 23_522);
/// assert_eq!(san_francisco_west.as_e4(), -1_224_194);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Longitude(i32);

impl Longitude {
    pub const MIN_E4: i32 = -180_0000;
    pub const MAX_E4: i32 = 180_0000;

    /// Creates a longitude from e4 fixed-point units.
    ///
    /// Valid range: `[-180_0000, 180_0000]`.
    pub fn try_from_e4(value: i32) -> Result<Self, GeoError> {
        if !(Self::MIN_E4..=Self::MAX_E4).contains(&value) {
            return Err(GeoError::InvalidLongitude { value });
        }

        Ok(Self(value))
    }

    /// Creates a longitude from decimal degrees.
    ///
    /// Values are rounded to the nearest e4 fixed-point unit.
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

    /// Returns the longitude as decimal degrees.
    pub fn as_degrees(self) -> f64 {
        self.0 as f64 / 10_000.0
    }

    fn to_degrees(self) -> f64 {
        self.as_degrees()
    }

    fn to_radians(self) -> f64 {
        self.as_degrees().to_radians()
    }
}

impl fmt::Display for Longitude {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}{DEGREE_SUFFIX}", self.to_degrees())
    }
}

impl FromStr for Longitude {
    type Err = GeoParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let degrees = parse_coordinate_degrees(s).map_err(GeoParseError::Longitude)?;
        Self::from_degrees(degrees).map_err(|error| GeoParseError::Longitude(error.to_string()))
    }
}

/// Location using validated latitude and longitude coordinates.
///
/// # Examples
///
/// ```
/// use netsim_core::geo::Location;
///
/// // Eastern longitude (Paris)
/// let paris = Location::from_degrees(48.8566, 2.3522).unwrap();
/// // Western longitude (San Francisco)
/// let san_francisco = Location::from_degrees(37.7749, -122.4194).unwrap();
///
/// assert!(paris.longitude.as_e4() > 0);
/// assert!(san_francisco.longitude.as_e4() < 0);
///
/// let parsed: Location = "48.8566, 2.3522".parse().unwrap();
/// assert_eq!(parsed.to_string(), "48.8566º, 2.3522º");
/// ```
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

impl TryFrom<(f64, f64)> for Location {
    type Error = GeoError;

    /// Converts `(latitude_degrees, longitude_degrees)` into a [`Location`].
    ///
    /// Delegates to [`Location::from_degrees`]; returns a [`GeoError`] if
    /// either value is out of range or non-finite.
    fn try_from((lat, lon): (f64, f64)) -> Result<Self, Self::Error> {
        Self::from_degrees(lat, lon)
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}", self.latitude, self.longitude)
    }
}

impl FromStr for Location {
    type Err = GeoParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(',');
        let Some(latitude_raw) = parts.next() else {
            return Err(GeoParseError::LocationMissingComponent);
        };
        let Some(longitude_raw) = parts.next() else {
            return Err(GeoParseError::LocationMissingComponent);
        };
        if parts.next().is_some() {
            return Err(GeoParseError::LocationExtraSeparator);
        }

        let latitude: Latitude = latitude_raw
            .trim()
            .parse()
            .map_err(|e| GeoParseError::LocationComponent("latitude", Box::new(e)))?;
        let longitude: Longitude = longitude_raw
            .trim()
            .parse()
            .map_err(|e| GeoParseError::LocationComponent("longitude", Box::new(e)))?;

        Ok(Self::new(latitude, longitude))
    }
}

/// Additional end-to-end efficiency factor applied to fiber propagation speed.
///
/// The effective speed used for latency computation is:
///
/// `effective_speed = SPEED_OF_FIBER * path_efficiency.as_ratio()`
///
/// where `path_efficiency` must be in `(0.0, 1.0]`.
///
/// # Parsing and display
///
/// ```
/// use netsim_core::geo::PathEfficiency;
///
/// let a: PathEfficiency = "75%".parse().unwrap();
/// let b: PathEfficiency = "0.75".parse().unwrap();
///
/// assert_eq!(a, b);
/// assert_eq!(a.to_string(), "75%");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct PathEfficiency(f64);

impl PathEfficiency {
    pub const FULL: Self = Self(1.0);
    pub const HALF: Self = Self(0.5);

    pub fn try_from_ratio(value: f64) -> Result<Self, GeoError> {
        if !value.is_finite() || value <= 0.0 || value > 1.0 {
            return Err(GeoError::InvalidPathEfficiency { value });
        }

        Ok(Self(value))
    }

    pub fn from_percent(value: f64) -> Result<Self, GeoError> {
        Self::try_from_ratio(value / 100.0)
    }

    pub const fn as_ratio(self) -> f64 {
        self.0
    }

    pub const fn as_percent(self) -> f64 {
        self.0 * 100.0
    }
}

impl Default for PathEfficiency {
    fn default() -> Self {
        Self::FULL
    }
}

impl TryFrom<f64> for PathEfficiency {
    type Error = GeoError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::try_from_ratio(value)
    }
}

impl TryFrom<u8> for PathEfficiency {
    type Error = GeoError;

    /// Converts an integer percentage in `[1, 100]` into a [`PathEfficiency`].
    ///
    /// `0` is rejected because zero efficiency is physically meaningless
    /// (it would imply infinite latency).
    fn try_from(percent: u8) -> Result<Self, Self::Error> {
        Self::from_percent(percent as f64)
    }
}

impl fmt::Display for PathEfficiency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Work in integer centipercent (percent * 100) to avoid heap allocation.
        // e.g. 0.5 → 5000 centipercent → "50%"
        //      0.755 → 7550 → "75.5%"
        //      0.3333 → 3333 → "33.33%"
        let centipercent = (self.as_percent() * 100.0).round() as u32;
        let whole = centipercent / 100;
        let frac = centipercent % 100;
        if frac == 0 {
            write!(f, "{whole}%")
        } else if frac.is_multiple_of(10) {
            write!(f, "{whole}.{}%", frac / 10)
        } else {
            write!(f, "{whole}.{frac:02}%")
        }
    }
}

impl FromStr for PathEfficiency {
    type Err = GeoParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(GeoParseError::PathEfficiency("empty".to_string()));
        }

        let ratio = if let Some(percent) = trimmed.strip_suffix('%') {
            let percent = percent.trim().parse::<f64>().map_err(|error| {
                GeoParseError::PathEfficiency(format!("invalid percent `{trimmed}`: {error}"))
            })?;
            percent / 100.0
        } else {
            trimmed.parse::<f64>().map_err(|error| {
                GeoParseError::PathEfficiency(format!("invalid ratio `{trimmed}`: {error}"))
            })?
        };

        Self::try_from_ratio(ratio)
            .map_err(|error| GeoParseError::PathEfficiency(error.to_string()))
    }
}

/// Speed of light in a vacuum (in meter per second)
const SPEED_OF_LIGHT: f64 = 299_792_458.0;
/// baseline propagation speed in fiber (69% of speed of light)
const SPEED_OF_FIBER: f64 = SPEED_OF_LIGHT * 0.69;

fn normalize_distance(distance: f64) -> Result<f64, GeoError> {
    if !distance.is_finite() || distance < 0.0 {
        return Err(GeoError::NonFiniteComputation);
    }

    Ok(distance)
}

// return the distance in meter between point1 and point2
fn distance_between_vincenty(point1: Location, point2: Location) -> Result<f64, GeoError> {
    let distance = VincentyInverse::default()
        .calculate(point1, point2, Spheroid::earth())
        .ok_or(GeoError::NonConvergent)?;

    normalize_distance(distance)
}

// The Earth Geodesic is constant (WGS-84 parameters never change). Cache it
// as a static so `Geodesic::new` — which computes ~42 polynomial coefficients —
// runs exactly once per process rather than on every distance/latency call.
static EARTH_GEODESIC: OnceLock<Geodesic> = OnceLock::new();

fn earth_geodesic() -> &'static Geodesic {
    EARTH_GEODESIC.get_or_init(|| {
        let s = Spheroid::earth();
        Geodesic::new(s.alpha, s.inv_flattening)
    })
}

// return the distance in meter between point1 and point2
fn distance_between_karney(point1: Location, point2: Location) -> Result<f64, GeoError> {
    let distance = earth_geodesic().inverse_distance(
        point1.latitude.to_degrees(),
        point1.longitude.to_degrees(),
        point2.latitude.to_degrees(),
        point2.longitude.to_degrees(),
    );

    if distance.is_finite() && distance >= 0.0 {
        normalize_distance(distance)
    } else {
        Err(GeoError::NonFiniteComputation)
    }
}

fn latency_from_distance(
    distance: f64,
    path_efficiency: PathEfficiency,
) -> Result<Latency, GeoError> {
    let latency_us = distance / (SPEED_OF_FIBER * path_efficiency.as_ratio()) * 1_000_000.0;

    if !latency_us.is_finite() || latency_us < 0.0 || latency_us > (u64::MAX as f64) {
        return Err(GeoError::NonFiniteComputation);
    }

    // Round to nearest microsecond to match Latency precision without systematic floor bias.
    let latency_us = latency_us.round() as u64;

    Ok(Latency::new(std::time::Duration::from_micros(latency_us)))
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

/// Reference distance using Vincenty inverse formula.
///
/// This is kept for algorithm comparison and testing.
#[doc(hidden)]
pub fn distance_between_locations_vincenty(p1: Location, p2: Location) -> Result<f64, GeoError> {
    distance_between_vincenty(p1, p2)
}

/// Distance between two locations using Karney inverse algorithm.
///
/// Returns the geodesic distance in meters.
///
/// # Example
///
/// ```
/// use netsim_core::geo::{Location, distance_between_locations};
///
/// // Eastern longitude (Paris) -> western longitude (San Francisco)
/// let paris = Location::from_degrees(48.8566, 2.3522).unwrap();
/// let san_francisco = Location::from_degrees(37.7749, -122.4194).unwrap();
///
/// let meters = distance_between_locations(paris, san_francisco).unwrap();
/// assert!(meters > 8_000_000.0);
/// ```
pub fn distance_between_locations(p1: Location, p2: Location) -> Result<f64, GeoError> {
    distance_between_locations_karney(p1, p2)
}

/// Distance using Karney inverse algorithm.
///
/// Alias for [`distance_between_locations`].
pub fn distance_between_locations_karney(p1: Location, p2: Location) -> Result<f64, GeoError> {
    distance_between_karney(p1, p2)
}

/// One-way propagation latency between two locations.
///
/// `path_efficiency` scales the baseline fiber propagation speed:
///
/// `effective_speed = SPEED_OF_FIBER * path_efficiency.as_ratio()`
///
/// - `1.0`: pure geometric fiber propagation (no extra slowdown)
/// - `0.5`: effective speed is halved, so latency doubles
///
/// This allows modeling additional path inefficiencies beyond straight-line
/// propagation (e.g. routing detours, switching/processing overhead).
///
/// Returns a [`Latency`] value with microsecond precision.
///
/// # Example
///
/// ```
/// use netsim_core::geo::{Location, PathEfficiency, latency_between_locations};
///
/// let paris = Location::from_degrees(48.8566, 2.3522).unwrap();
/// let london = Location::from_degrees(51.5074, -0.1278).unwrap();
/// let efficiency: PathEfficiency = "80%".parse().unwrap();
///
/// let latency = latency_between_locations(paris, london, efficiency).unwrap();
/// assert!(latency.into_duration().as_micros() > 0);
/// ```
///
/// Alias for [`latency_between_locations_karney`].
pub fn latency_between_locations(
    p1: Location,
    p2: Location,
    path_efficiency: PathEfficiency,
) -> Result<Latency, GeoError> {
    latency_between_locations_karney(p1, p2, path_efficiency)
}

/// Latency using Karney/GeographicLib inverse distance.
///
/// Alias for [`latency_between_locations`].
pub fn latency_between_locations_karney(
    p1: Location,
    p2: Location,
    path_efficiency: PathEfficiency,
) -> Result<Latency, GeoError> {
    let distance = distance_between_locations_karney(p1, p2)?;
    latency_from_distance(distance, path_efficiency)
}

/// Reference latency using Vincenty inverse distance.
///
/// This is kept for algorithm comparison and testing.
#[doc(hidden)]
pub fn latency_between_locations_vincenty(
    p1: Location,
    p2: Location,
    path_efficiency: PathEfficiency,
) -> Result<Latency, GeoError> {
    let distance = distance_between_vincenty(p1, p2)?;
    latency_from_distance(distance, path_efficiency)
}

const DEGREE_SUFFIX: char = '\u{00BA}';
const ALT_DEGREE_SUFFIX: char = '\u{00B0}';

fn parse_coordinate_degrees(input: &str) -> Result<f64, String> {
    let trimmed = input.trim();
    let trimmed = trimmed
        .strip_suffix(DEGREE_SUFFIX)
        .or_else(|| trimmed.strip_suffix(ALT_DEGREE_SUFFIX))
        .unwrap_or(trimmed)
        .trim();

    if trimmed.is_empty() {
        return Err("cannot parse from empty string".to_string());
    }

    trimmed
        .parse::<f64>()
        .map_err(|error| format!("failed to parse `{input}`: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATH_EFFICIENCY: PathEfficiency = PathEfficiency::HALF;

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
        let latency = latency_between_locations(p1(), p2(), PATH_EFFICIENCY).unwrap();

        assert_eq!(latency.to_string(), "122ms512µs");
    }

    #[test]
    fn latency_between_self() {
        let p1 = p1();
        let latency = latency_between_locations(p1, p1, PATH_EFFICIENCY).unwrap();

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
    fn rejects_invalid_path_efficiency() {
        assert_eq!(
            PathEfficiency::try_from_ratio(0.0).unwrap_err(),
            GeoError::InvalidPathEfficiency { value: 0.0 }
        );
    }

    #[test]
    fn path_efficiency_try_from_u8_valid() {
        // 100% → FULL
        assert_eq!(
            PathEfficiency::try_from(100u8).unwrap(),
            PathEfficiency::FULL
        );
        // 50% → HALF
        assert_eq!(
            PathEfficiency::try_from(50u8).unwrap(),
            PathEfficiency::HALF
        );
        // try_into() spelling
        let via_into: PathEfficiency = 75u8.try_into().unwrap();
        assert!((via_into.as_percent() - 75.0).abs() < 1e-6);
        // boundary: 1% is the lowest valid value
        assert!(PathEfficiency::try_from(1u8).is_ok());
    }

    #[test]
    fn path_efficiency_try_from_u8_zero_rejected() {
        assert!(matches!(
            PathEfficiency::try_from(0u8),
            Err(GeoError::InvalidPathEfficiency { .. })
        ));
    }

    #[test]
    fn rejects_non_finite_coordinate_degrees() {
        assert_eq!(
            Location::from_degrees(f64::NAN, 0.0).unwrap_err(),
            GeoError::NonFiniteComputation
        );
    }

    #[test]
    #[allow(clippy::zero_prefixed_literal)] // e4 fixed-point coords: `0_0100` reads as 0.0100°
    fn short_distance_keeps_microsecond_precision() {
        let p1 = Location::try_from_e4(0, 0).unwrap();
        let p2 = Location::try_from_e4(0_0100, 0).unwrap();

        let latency = latency_between_locations(p1, p2, PathEfficiency::FULL).unwrap();
        let duration = latency.into_duration();
        assert!(duration > std::time::Duration::ZERO);
        assert!(duration < std::time::Duration::from_millis(1));
    }

    #[test]
    fn latency_is_symmetric() {
        let p1 = p1();
        let p2 = p2();

        let forward = latency_between_locations(p1, p2, PATH_EFFICIENCY).unwrap();
        let backward = latency_between_locations(p2, p1, PATH_EFFICIENCY).unwrap();

        assert_eq!(forward, backward);
    }

    #[test]
    fn antipodal_points_can_fail_to_converge() {
        let p1 = Location::try_from_e4(0, 0).unwrap();
        let p2 = Location::try_from_e4(0, 180_0000).unwrap();

        assert_eq!(
            distance_between_locations_vincenty(p1, p2).unwrap_err(),
            GeoError::NonConvergent
        );
    }

    #[test]
    fn karney_inverse_handles_antipodal_points() {
        let p1 = Location::try_from_e4(0, 0).unwrap();
        let p2 = Location::try_from_e4(0, 180_0000).unwrap();

        let distance = distance_between_locations_karney(p1, p2).unwrap();
        assert!((distance - 20_003_931.0).abs() < 1.0);
    }

    #[test]
    fn karney_and_vincenty_are_close_on_regular_case() {
        let p1 = p1();
        let p2 = p2();

        let vincenty = distance_between_locations_vincenty(p1, p2).unwrap();
        let karney = distance_between_locations_karney(p1, p2).unwrap();

        assert!((vincenty - karney).abs() < 1.0);
    }

    #[test]
    fn default_distance_and_latency_use_karney_for_antipodal_points() {
        let p1 = Location::try_from_e4(0, 0).unwrap();
        let p2 = Location::try_from_e4(0, 180_0000).unwrap();

        let distance = distance_between_locations(p1, p2).unwrap();
        let karney_distance = distance_between_locations_karney(p1, p2).unwrap();
        assert!((distance - karney_distance).abs() < 1e-6);

        let latency = latency_between_locations(p1, p2, PATH_EFFICIENCY).unwrap();
        let karney_latency = latency_between_locations_karney(p1, p2, PATH_EFFICIENCY).unwrap();
        assert_eq!(latency, karney_latency);
    }

    #[test]
    fn path_efficiency_display_and_parse() {
        assert_eq!(PathEfficiency::HALF.to_string(), "50%");
        assert_eq!(
            "50%".parse::<PathEfficiency>().unwrap(),
            PathEfficiency::HALF
        );
        assert_eq!(
            " 50 % ".parse::<PathEfficiency>().unwrap(),
            PathEfficiency::HALF
        );
        assert_eq!(
            "0.5".parse::<PathEfficiency>().unwrap(),
            PathEfficiency::HALF
        );
    }

    #[test]
    fn path_efficiency_parse_rejects_invalid_values() {
        assert_eq!(
            PathEfficiency::try_from_ratio(1.1).unwrap_err(),
            GeoError::InvalidPathEfficiency { value: 1.1 }
        );
        assert_eq!(
            PathEfficiency::try_from_ratio(-0.1).unwrap_err(),
            GeoError::InvalidPathEfficiency { value: -0.1 }
        );
        assert!("abc".parse::<PathEfficiency>().is_err());
    }

    #[test]
    fn latitude_as_degrees() {
        let lat = Latitude::try_from_e4(48_8534).unwrap();
        assert!((lat.as_degrees() - 48.8534).abs() < 1e-4);

        let lat_neg = Latitude::try_from_e4(-90_0000).unwrap();
        assert_eq!(lat_neg.as_degrees(), -90.0);

        let lat_zero = Latitude::try_from_e4(0).unwrap();
        assert_eq!(lat_zero.as_degrees(), 0.0);
    }

    #[test]
    fn longitude_as_degrees() {
        let lon = Longitude::try_from_e4(-122_4194).unwrap();
        assert!((lon.as_degrees() - -122.4194).abs() < 1e-4);

        let lon_max = Longitude::try_from_e4(180_0000).unwrap();
        assert_eq!(lon_max.as_degrees(), 180.0);

        let lon_zero = Longitude::try_from_e4(0).unwrap();
        assert_eq!(lon_zero.as_degrees(), 0.0);
    }

    #[test]
    fn latitude_display_and_parse() {
        let latitude = Latitude::try_from_e4(48_8534).unwrap();

        assert_eq!(latitude.to_string(), "48.8534\u{00BA}");
        assert_eq!("48.8534".parse::<Latitude>().unwrap(), latitude);
        assert_eq!("48.8534\u{00BA}".parse::<Latitude>().unwrap(), latitude);
        assert_eq!("48.8534\u{00B0}".parse::<Latitude>().unwrap(), latitude);
    }

    #[test]
    fn longitude_display_and_parse() {
        let longitude = Longitude::try_from_e4(-122_4194).unwrap();

        assert_eq!(longitude.to_string(), "-122.4194\u{00BA}");
        assert_eq!("-122.4194".parse::<Longitude>().unwrap(), longitude);
        assert_eq!(
            " -122.4194\u{00BA} ".parse::<Longitude>().unwrap(),
            longitude
        );
        assert_eq!("-122.4194\u{00B0}".parse::<Longitude>().unwrap(), longitude);
    }

    #[test]
    fn coordinate_parse_rejects_invalid_values() {
        let latitude_err = "invalid".parse::<Latitude>().unwrap_err().to_string();
        assert!(latitude_err.contains("Failed to parse Latitude"));

        let longitude_err = "181".parse::<Longitude>().unwrap_err().to_string();
        assert!(longitude_err.contains("longitude out of range"));
    }

    #[test]
    fn coordinate_display_roundtrip() {
        let latitude = Latitude::try_from_e4(-49_3523).unwrap();
        let longitude = Longitude::try_from_e4(70_2150).unwrap();

        assert_eq!(latitude.to_string().parse::<Latitude>().unwrap(), latitude);
        assert_eq!(
            longitude.to_string().parse::<Longitude>().unwrap(),
            longitude
        );
    }

    #[test]
    fn location_display_and_parse() {
        let location = Location::try_from_e4(48_8566, 2_3522).unwrap();

        assert_eq!(location.to_string(), "48.8566º, 2.3522º");
        assert_eq!("48.8566, 2.3522".parse::<Location>().unwrap(), location);
        assert_eq!("48.8566º, 2.3522º".parse::<Location>().unwrap(), location);
    }

    #[test]
    fn location_parse_rejects_invalid_values() {
        let missing_separator = "48.8566".parse::<Location>().unwrap_err().to_string();
        assert!(missing_separator.contains("expected format"));

        let extra_separator = "48.8566, 2.3522, 1"
            .parse::<Location>()
            .unwrap_err()
            .to_string();
        assert!(extra_separator.contains("single comma"));

        let invalid_longitude = "48.8566, 181".parse::<Location>().unwrap_err().to_string();
        assert!(invalid_longitude.contains("Failed to parse Location longitude"));
    }

    #[test]
    fn location_display_roundtrip() {
        let location = Location::try_from_e4(-49_3523, 70_2150).unwrap();

        assert_eq!(location.to_string().parse::<Location>().unwrap(), location);
    }

    #[test]
    fn location_try_from_f64_tuple_valid() {
        let expected = Location::from_degrees(48.8566, 2.3522).unwrap();
        let actual = Location::try_from((48.8566_f64, 2.3522_f64)).unwrap();
        assert_eq!(actual, expected);

        // try_into() spelling
        let via_into: Location = (48.8566_f64, 2.3522_f64).try_into().unwrap();
        assert_eq!(via_into, expected);
    }

    #[test]
    fn location_try_from_f64_tuple_invalid_latitude() {
        assert!(matches!(
            Location::try_from((91.0_f64, 0.0_f64)),
            Err(GeoError::InvalidLatitude { .. })
        ));
    }

    #[test]
    fn location_try_from_f64_tuple_invalid_longitude() {
        assert!(matches!(
            Location::try_from((0.0_f64, 181.0_f64)),
            Err(GeoError::InvalidLongitude { .. })
        ));
    }

    #[test]
    fn location_try_from_f64_tuple_non_finite() {
        assert!(matches!(
            Location::try_from((f64::NAN, 0.0_f64)),
            Err(GeoError::NonFiniteComputation)
        ));
        assert!(matches!(
            Location::try_from((0.0_f64, f64::INFINITY)),
            Err(GeoError::NonFiniteComputation)
        ));
    }
}
