use crate::Latency;

/// Location using Latitude and Longitude
pub type Location = (i64, u64);

fn location_to_radian((latitude, longitude): (i64, u64)) -> (f64, f64) {
    // latitude is between 90 South and 90 North with a precision of 4 digits
    // longitude is between 0 and 180 degres with a precision of 4 digits
    assert!((-90_0000..=90_0000).contains(&latitude));
    assert!(longitude <= 180_0000);

    (
        (latitude as f64 / 10000.0).to_radians(),
        (longitude as f64 / 10000.0).to_radians(),
    )
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
pub struct Spheroid {
    /// Semi Major Axis in meter / Radius at equator
    alpha: f64,
    /// Semi Minor axis in meter / Radius at pole
    beta: f64,
    /// inverse flattening 1/f
    inv_flattening: f64,
}

impl Spheroid {
    #[allow(unused)]
    pub fn new(alpha: f64, beta: f64, inv_flattening: f64) -> Self {
        Self {
            alpha,
            beta,
            inv_flattening,
        }
    }

    pub const fn earth() -> Self {
        Self {
            alpha: 6378137.0,
            beta: 6356752.314245,
            inv_flattening: 0.00335281066, // 1.0 / 298.257223563,
        }
    }
}

/// Vincenty inverse formula, parametrized with the number of maximum iterations
/// for the algorithm
///
/// [Wikipedia Vincenty formulae](https://en.wikipedia.org/wiki/Vincenty%27s_formulae)
#[derive(Clone, Debug)]
pub struct VincentyInverse {
    pub nb_iter: usize,
}

impl Default for VincentyInverse {
    fn default() -> Self {
        Self { nb_iter: 50 }
    }
}

pub trait SpheroidDistanceAlgorithm {
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

        let p1 = location_to_radian(point1);
        let p2 = location_to_radian(point2);

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
