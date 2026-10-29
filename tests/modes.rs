//! Cada modo entrega todos los mensajes exactamente una vez. Sin io_uring
//! o sin MSG_RING estos tests FALLAN (no se saltean).

use msgring_bench::modes::{check_msgring_supported, run, Mode, Params, Spec};

fn params() -> Params {
    Params {
        queue_capacity: 64,
        consumer_cq_entries: 4096,
        producer_ring_entries: 16,
        consumer_cpu: 0,
        producer_cpus_start: 1,
        pin: false,
    }
}

fn check(mode: Mode, p: usize, gap: u64) {
    let spec = Spec {
        mode,
        producers: p,
        msgs_per_producer: 3000,
        gap_ns: gap,
        warmup_per_producer: 100,
    };
    let r = run(&spec, &params()).expect("la corrida tiene que poder ejecutarse");
    assert_eq!(r.received, p * 3000, "{mode:?}");
    assert_eq!(
        (r.lost, r.duplicated, r.msgring_errors),
        (0, 0, 0),
        "{mode:?}"
    );
    assert_eq!(r.latencies.len(), p * 2900);
    assert!(r.latencies.windows(2).all(|w| w[0] <= w[1]), "ordenadas");
    assert!(r.msgs_per_s > 0.0);
}

#[test]
fn msgring_is_supported_here() {
    check_msgring_supported().expect("io_uring con MSG_RING es obligatorio");
}

#[test]
fn every_mode_delivers_exactly_once_single_producer_paced() {
    for m in ["msgring", "msgring_defer", "mpmc_eventfd", "mpmc_spin"] {
        check(Mode::parse(m).unwrap(), 1, 10_000);
    }
}

#[test]
fn every_mode_delivers_exactly_once_three_producers_unpaced() {
    // Cola de 64 para forzar reintentos de push con la cola llena.
    for m in ["msgring", "msgring_defer", "mpmc_eventfd", "mpmc_spin"] {
        check(Mode::parse(m).unwrap(), 3, 0);
    }
}

#[test]
fn unknown_mode_is_rejected() {
    assert!(Mode::parse("x").is_none());
}
