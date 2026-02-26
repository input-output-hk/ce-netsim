use criterion::{Criterion, black_box, criterion_group, criterion_main};
use netsim_core::geo::{self, Location};

const SOL_FO: f64 = 0.5;

fn p1() -> Location {
    // 48.853415254543435, 2.3487911014845038
    Location::try_from_e4(48_8534, 2_3487).expect("benchmark coordinate must be valid")
}

fn p2() -> Location {
    // -49.35231574277824, 70.2150600748867
    Location::try_from_e4(-49_3523, 70_2150).expect("benchmark coordinate must be valid")
}

fn vincenty(c: &mut Criterion) {
    let p1 = p1();
    let p2 = p2();

    let v = black_box(geo::latency_between_locations(
        black_box(p1),
        black_box(p2),
        SOL_FO,
    ));

    println!("{v:?}");

    c.bench_function("geo::latency_between_locations", |b| {
        b.iter(|| {
            black_box(geo::latency_between_locations(
                black_box(p1),
                black_box(p2),
                SOL_FO,
            ))
        });
    });
}

criterion_group!(benches, vincenty);
criterion_main!(benches);
