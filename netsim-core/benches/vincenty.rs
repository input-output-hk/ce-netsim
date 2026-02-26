use criterion::{Criterion, black_box, criterion_group, criterion_main};
use netsim_core::geo::{self, Location};

//48.853415254543435, 2.3487911014845038
const P1: Location = (48_8534, 2_3487);
// -49.35231574277824, 70.2150600748867
const P2: Location = (-49_3523, 70_2150);
const SOL_FO: f64 = 0.5;

fn vincenty(c: &mut Criterion) {
    let v = black_box(geo::latency_between_locations(
        black_box(P1),
        black_box(P2),
        SOL_FO,
    ));

    println!("{v:?}");

    c.bench_function("geo::latency_between_locations", |b| {
        b.iter(|| {
            black_box(geo::latency_between_locations(
                black_box(P1),
                black_box(P2),
                SOL_FO,
            ))
        });
    });
}

criterion_group!(benches, vincenty);
criterion_main!(benches);
