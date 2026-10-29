use msgring_bench::Design;
use sha2::{Digest, Sha256};

fn read(p: &str) -> String {
    std::fs::read_to_string(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), p)).unwrap()
}

#[test]
fn design_matches_preregistration_hash() {
    for line in read("PREREGISTRO.sha256").lines() {
        let (hash, path) = line.split_once("  ").unwrap();
        let got: String = Sha256::digest(read(path).as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(got, hash, "{path} cambió después del pre-registro");
    }
}

#[test]
fn design_parses_with_preregistered_values() {
    let d = Design::parse(&read("design.json")).unwrap();
    assert_eq!(d.modes.len(), 4);
    assert_eq!(d.producers, vec![1, 7]);
    assert_eq!((d.reps, d.aa_reps), (10, 10));
    assert_eq!(
        (d.lat_msgs, d.lat_gap_ns, d.lat_warmup),
        (10_000, 50_000, 1_000)
    );
    assert_eq!(d.tput_msgs, 50_000);
    assert_eq!(
        (d.order_seed, d.boot_resamples, d.boot_seed),
        (20261029, 10_000, 7)
    );
}
