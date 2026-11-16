//! Probe: self-query recall of a raw usearch i8 index on random unit vectors, by search effort.
//! cargo test -p search-vector --release --test recall_probe -- --ignored --nocapture
mod common;
use search_vector::constants::EMBED_DIM;
use usearch::{ffi::{IndexOptions, MetricKind, ScalarKind}, Index};

fn unit(text: &str) -> Vec<f32> {
    let mut v = common::vector_for(text, EMBED_DIM);
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.iter_mut().for_each(|x| *x /= n);
    v
}

#[test]
#[ignore = "probe"]
fn recall_by_effort() {
    let n = 200_000u64;
    for quant in [ScalarKind::I8, ScalarKind::F32] {
        let opts = IndexOptions { dimensions: EMBED_DIM, metric: MetricKind::Cos, quantization: quant, connectivity: 0, expansion_add: 0, expansion_search: 0, multi: false };
        let ix = Index::new(&opts).unwrap();
        ix.reserve(n as usize).unwrap();
        std::thread::scope(|s| for t in 0..8u64 {
            let ix = &ix;
            s.spawn(move || { let mut i = t; while i < n { ix.add(i, &unit(&format!("m{i}"))).unwrap(); i += 8; } });
        });
        for ef in [0usize, 128, 256, 512] {
            if ef > 0 { ix.change_expansion_search(ef); }
            let (mut top1, mut exact_top1) = (0, 0);
            for q in 0..200u64 {
                let k = q * (n / 200);
                let qv = unit(&format!("m{k}"));
                top1 += usize::from(ix.search(&qv, 1).unwrap().keys.first() == Some(&k));
                exact_top1 += usize::from(ix.exact_search(&qv, 1).unwrap().keys.first() == Some(&k));
            }
            println!("PROBE quant={quant:?} n={n} ef={} (default if 0; library says {}) hnsw_top1={top1}/200 exact_top1={exact_top1}/200", ef, ix.expansion_search());
        }
    }
}
