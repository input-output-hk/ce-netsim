use criterion::{Criterion, black_box, criterion_group, criterion_main};
use netsim_core::geo::{self, Location};

const SOL_FO: f64 = 0.5;
const P1_LAT_E4: i32 = 48_8534;
const P1_LON_E4: i32 = 2_3487;
const P2_LAT_E4: i32 = -49_3523;
const P2_LON_E4: i32 = 70_2150;

fn p1() -> Location {
    // 48.853415254543435, 2.3487911014845038
    Location::try_from_e4(P1_LAT_E4, P1_LON_E4).expect("benchmark coordinate must be valid")
}

fn p2() -> Location {
    // -49.35231574277824, 70.2150600748867
    Location::try_from_e4(P2_LAT_E4, P2_LON_E4).expect("benchmark coordinate must be valid")
}

fn vincenty(c: &mut Criterion) {
    let p1 = p1();
    let p2 = p2();

    c.bench_function("geo::distance_between_locations_vincenty", |b| {
        b.iter(|| {
            black_box(geo::distance_between_locations_vincenty(
                black_box(p1),
                black_box(p2),
            ))
        });
    });

    c.bench_function("geo::distance_between_locations_karney", |b| {
        b.iter(|| {
            black_box(geo::distance_between_locations_karney(
                black_box(p1),
                black_box(p2),
            ))
        });
    });

    c.bench_function("geo::latency_between_locations_vincenty", |b| {
        b.iter(|| {
            black_box(geo::latency_between_locations(
                black_box(p1),
                black_box(p2),
                SOL_FO,
            ))
        });
    });

    c.bench_function("geo::latency_between_locations_karney", |b| {
        b.iter(|| {
            black_box(geo::latency_between_locations_karney(
                black_box(p1),
                black_box(p2),
                SOL_FO,
            ))
        });
    });
}

criterion_group!(benches, vincenty);
criterion_main!(benches);
