use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use netsim_core::geo::{self, GeoError, Location};

const FIBER_SPEED_RATIO: f64 = 0.5;

#[derive(Clone, Copy)]
struct GeoCase {
    name: &'static str,
    p1: Location,
    p2: Location,
}

fn location(latitude_e4: i32, longitude_e4: i32) -> Location {
    Location::try_from_e4(latitude_e4, longitude_e4).expect("benchmark coordinate must be valid")
}

fn benchmark_cases() -> [GeoCase; 5] {
    [
        GeoCase {
            name: "local_short",
            // San Francisco downtown local hop (~140m)
            p1: location(37_7749, -122_4194),
            p2: location(37_7759, -122_4184),
        },
        GeoCase {
            name: "regional",
            // Paris <-> London
            p1: location(48_8566, 2_3522),
            p2: location(51_5074, -0_1278),
        },
        GeoCase {
            name: "long_haul_intercontinental",
            // New York <-> Sydney
            p1: location(40_7128, -74_0060),
            p2: location(-33_8688, 151_2093),
        },
        GeoCase {
            name: "near_antipodal_stress",
            // Near-antipodal but not exact
            p1: location(10_0000, 20_0000),
            p2: location(-10_0001, -159_9999),
        },
        GeoCase {
            name: "exact_antipodal",
            p1: location(0, 0),
            p2: location(0, 180_0000),
        },
    ]
}

fn format_f64_result(result: &Result<f64, GeoError>) -> String {
    match result {
        Ok(value) => format!("{value:.3}"),
        Err(error) => format!("err({error})"),
    }
}

fn format_u128_result(result: &Result<u128, GeoError>) -> String {
    match result {
        Ok(value) => value.to_string(),
        Err(error) => format!("err({error})"),
    }
}

fn emit_comparison_report(cases: &[GeoCase]) {
    eprintln!("geo benchmark comparison snapshot");
    eprintln!("case,vincenty_m,karney_m,abs_delta_m,vincenty_us,karney_us,abs_delta_us");

    for case in cases {
        let vincenty_distance = geo::distance_between_locations_vincenty(case.p1, case.p2);
        let karney_distance = geo::distance_between_locations_karney(case.p1, case.p2);
        let vincenty_latency =
            geo::latency_between_locations_vincenty(case.p1, case.p2, FIBER_SPEED_RATIO)
                .map(|latency| latency.into_duration().as_micros());
        let karney_latency =
            geo::latency_between_locations_karney(case.p1, case.p2, FIBER_SPEED_RATIO)
                .map(|latency| latency.into_duration().as_micros());

        let distance_delta = match (&vincenty_distance, &karney_distance) {
            (Ok(v), Ok(k)) => format!("{:.6}", (v - k).abs()),
            _ => "-".to_string(),
        };

        let latency_delta = match (&vincenty_latency, &karney_latency) {
            (Ok(v), Ok(k)) => v.abs_diff(*k).to_string(),
            _ => "-".to_string(),
        };

        eprintln!(
            "{},{},{},{},{},{},{}",
            case.name,
            format_f64_result(&vincenty_distance),
            format_f64_result(&karney_distance),
            distance_delta,
            format_u128_result(&vincenty_latency),
            format_u128_result(&karney_latency),
            latency_delta,
        );
    }
}

fn bench_distance(c: &mut Criterion, cases: &[GeoCase]) {
    let mut group = c.benchmark_group("geo::distance");

    for case in cases {
        let p1 = case.p1;
        let p2 = case.p2;
        group.bench_function(BenchmarkId::new("vincenty", case.name), |b| {
            b.iter(|| {
                black_box(geo::distance_between_locations_vincenty(
                    black_box(p1),
                    black_box(p2),
                ))
            })
        });
        group.bench_function(BenchmarkId::new("karney", case.name), |b| {
            b.iter(|| {
                black_box(geo::distance_between_locations_karney(
                    black_box(p1),
                    black_box(p2),
                ))
            })
        });
    }

    group.finish();
}

fn bench_latency(c: &mut Criterion, cases: &[GeoCase]) {
    let mut group = c.benchmark_group("geo::latency");

    for case in cases {
        let p1 = case.p1;
        let p2 = case.p2;
        group.bench_function(BenchmarkId::new("vincenty", case.name), |b| {
            b.iter(|| {
                black_box(geo::latency_between_locations_vincenty(
                    black_box(p1),
                    black_box(p2),
                    FIBER_SPEED_RATIO,
                ))
            })
        });
        group.bench_function(BenchmarkId::new("karney", case.name), |b| {
            b.iter(|| {
                black_box(geo::latency_between_locations_karney(
                    black_box(p1),
                    black_box(p2),
                    FIBER_SPEED_RATIO,
                ))
            })
        });
    }

    group.finish();
}

fn geo(c: &mut Criterion) {
    let cases = benchmark_cases();
    emit_comparison_report(&cases);
    bench_distance(c, &cases);
    bench_latency(c, &cases);
}

criterion_group!(benches, geo);
criterion_main!(benches);
