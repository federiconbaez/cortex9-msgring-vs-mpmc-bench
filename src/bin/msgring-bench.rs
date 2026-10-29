//! uso:
//!   msgring-bench smoke                         corrida corta de los 4 modos; falla sin io_uring/MSG_RING
//!   msgring-bench run <design.json> <runs.csv>  medición completa según el diseño
//!   msgring-bench analyze <design.json> <runs.csv>  bootstrap e IC 95 %

use msgring_bench::modes::{self, Mode, Spec};
use msgring_bench::stats::{bootstrap_diff, percentile_sorted, shuffle};
use msgring_bench::Design;
use std::collections::BTreeMap;
use std::io::Write;
use std::process::ExitCode;

fn smoke() -> ExitCode {
    if let Err(e) = modes::check_msgring_supported() {
        eprintln!("SMOKE FALLA: {e}");
        return ExitCode::from(1);
    }
    let ncpu = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let params = modes::Params {
        queue_capacity: 1024,
        consumer_cq_entries: 4096,
        producer_ring_entries: 64,
        consumer_cpu: 0,
        producer_cpus_start: 1,
        pin: ncpu >= 3,
    };
    let mut ok = true;
    for m in ["msgring", "msgring_defer", "mpmc_eventfd", "mpmc_spin"] {
        for (p, gap) in [(1, 20_000), (2, 0)] {
            let spec = Spec {
                mode: Mode::parse(m).unwrap(),
                producers: p,
                msgs_per_producer: 2000,
                gap_ns: gap,
                warmup_per_producer: 200,
            };
            match modes::run(&spec, &params) {
                Ok(r) => {
                    let line = format!(
                        "{m} P={p} gap={gap} recibidos={} perdidos={} dup={} err={} valid={}",
                        r.received,
                        r.lost,
                        r.duplicated,
                        r.msgring_errors,
                        r.valid()
                    );
                    println!("{line}");
                    ok &= r.valid() && r.received == p * 2000;
                }
                Err(e) => {
                    println!("{m} P={p}: ERROR {e}");
                    ok = false;
                }
            }
        }
    }
    println!("(smoke: los números de latencia no son la medición)");
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

struct Job {
    phase: &'static str,
    mode: String,
    producers: usize,
    rep: usize,
    batch: &'static str,
}

fn run_all(design_path: &str, out: &str) -> ExitCode {
    let d = Design::parse(&std::fs::read_to_string(design_path).unwrap()).unwrap();
    if let Err(e) = modes::check_msgring_supported() {
        eprintln!("{e}");
        return ExitCode::from(1);
    }
    let mut jobs = vec![];
    for phase in ["latency", "throughput"] {
        for m in &d.modes {
            for &p in &d.producers {
                for rep in 0..d.reps {
                    jobs.push(Job {
                        phase,
                        mode: m.clone(),
                        producers: p,
                        rep,
                        batch: "main",
                    });
                }
            }
        }
    }
    let aa_mode = d.raw["aa_cell"]["mode"].as_str().unwrap().to_string();
    let aa_p = d.raw["aa_cell"]["producers"].as_u64().unwrap() as usize;
    for rep in 0..d.aa_reps {
        jobs.push(Job {
            phase: "latency",
            mode: aa_mode.clone(),
            producers: aa_p,
            rep,
            batch: "aa",
        });
    }
    shuffle(&mut jobs, d.order_seed);
    let mut f = std::fs::File::create(out).unwrap();
    writeln!(f, "order,phase,mode,producers,rep,batch,n_samples,p50_ns,p99_ns,p999_ns,msgs_per_s,received,lost,dup,msgring_errors,valid,steal_ticks,load1").unwrap();
    for (i, j) in jobs.iter().enumerate() {
        let spec = Spec {
            mode: Mode::parse(&j.mode).unwrap(),
            producers: j.producers,
            msgs_per_producer: if j.phase == "latency" {
                d.lat_msgs
            } else {
                d.tput_msgs
            },
            gap_ns: if j.phase == "latency" {
                d.lat_gap_ns
            } else {
                0
            },
            warmup_per_producer: if j.phase == "latency" {
                d.lat_warmup
            } else {
                0
            },
        };
        let load = modes::loadavg1();
        let s0 = modes::steal_ticks();
        let r = match modes::run(&spec, &d.params) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("corrida {i} no pudo ejecutarse: {e}");
                return ExitCode::from(1);
            }
        };
        let steal = modes::steal_ticks() - s0;
        let (p50, p99, p999) = if r.latencies.is_empty() || j.phase != "latency" {
            (0, 0, 0)
        } else {
            (
                percentile_sorted(&r.latencies, 0.50),
                percentile_sorted(&r.latencies, 0.99),
                percentile_sorted(&r.latencies, 0.999),
            )
        };
        writeln!(
            f,
            "{i},{},{},{},{},{},{},{p50},{p99},{p999},{:.0},{},{},{},{},{},{steal},{load}",
            j.phase,
            j.mode,
            j.producers,
            j.rep,
            j.batch,
            r.latencies.len(),
            r.msgs_per_s,
            r.received,
            r.lost,
            r.duplicated,
            r.msgring_errors,
            r.valid()
        )
        .unwrap();
        eprintln!(
            "[{}/{}] {} {} P={} {} p99={}us msg/s={:.0} valid={}",
            i + 1,
            jobs.len(),
            j.phase,
            j.mode,
            j.producers,
            j.batch,
            p99 / 1000,
            r.msgs_per_s,
            r.valid()
        );
    }
    ExitCode::SUCCESS
}

type Row = BTreeMap<String, String>;

fn read_csv(path: &str) -> Vec<Row> {
    let s = std::fs::read_to_string(path).unwrap();
    let mut lines = s.lines();
    let hdr: Vec<&str> = lines.next().unwrap().split(',').collect();
    lines
        .map(|l| {
            hdr.iter()
                .map(|h| h.to_string())
                .zip(l.split(',').map(String::from))
                .collect()
        })
        .collect()
}

fn values(rows: &[Row], phase: &str, mode: &str, p: usize, batch: &str, metric: &str) -> Vec<f64> {
    rows.iter()
        .filter(|r| {
            r["phase"] == phase
                && r["mode"] == mode
                && r["producers"] == p.to_string()
                && r["batch"] == batch
                && r["valid"] == "true"
        })
        .map(|r| r[metric].parse::<f64>().unwrap())
        .collect()
}

fn analyze(design_path: &str, csv: &str) -> ExitCode {
    let d = Design::parse(&std::fs::read_to_string(design_path).unwrap()).unwrap();
    let rows = read_csv(csv);
    let invalid = rows.iter().filter(|r| r["valid"] != "true").count();
    let mut cmp_idx = 0u64;
    let mut ci = |a: &[f64], b: &[f64]| {
        cmp_idx += 1;
        bootstrap_diff(
            a,
            b,
            d.boot_resamples,
            d.boot_seed.wrapping_add(cmp_idx),
            0.95,
        )
    };
    let us = |x: f64| x / 1000.0;
    println!("# Análisis\n");
    println!("Corridas: {} (inválidas: {invalid}). Bootstrap: {} remuestreos, semilla base {}, IC 95 %.\n", rows.len(), d.boot_resamples, d.boot_seed);
    let steal: u64 = rows
        .iter()
        .map(|r| r["steal_ticks"].parse::<u64>().unwrap_or(0))
        .sum();
    let loads: Vec<f64> = rows
        .iter()
        .filter_map(|r| r["load1"].parse().ok())
        .collect();
    println!(
        "Ruido: steal total {steal} ticks; loadavg 1 min por corrida entre {:.2} y {:.2}.\n",
        loads.iter().cloned().fold(f64::INFINITY, f64::min),
        loads.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    );
    println!("## Medias por celda (latencia en µs, media de los valores por corrida)\n");
    println!("| modo | P | n | p50 | p99 | p99.9 | msg/s (throughput) |");
    println!("|---|---|---|---|---|---|---|");
    for m in &d.modes {
        for &p in &d.producers {
            let p50 = values(&rows, "latency", m, p, "main", "p50_ns");
            let p99 = values(&rows, "latency", m, p, "main", "p99_ns");
            let p999 = values(&rows, "latency", m, p, "main", "p999_ns");
            let tp = values(&rows, "throughput", m, p, "main", "msgs_per_s");
            let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
            println!(
                "| {m} | {p} | {} | {:.1} | {:.1} | {:.1} | {:.0} |",
                p99.len(),
                us(mean(&p50)),
                us(mean(&p99)),
                us(mean(&p999)),
                mean(&tp)
            );
        }
    }
    let mut all_ok = true;
    let verdict = |c: &msgring_bench::stats::Ci, a: &str, b: &str| {
        if !c.excludes_zero() {
            "IC cruza 0: sin ganador".to_string()
        } else if c.diff < 0.0 {
            format!("gana {a} (menor)")
        } else {
            format!("gana {b} (menor)")
        }
    };
    println!("\n## Controles\n");
    let nc = ci(
        &values(&rows, "latency", "mpmc_eventfd", 1, "main", "p99_ns"),
        &values(&rows, "latency", "mpmc_eventfd", 1, "aa", "p99_ns"),
    );
    let nc_ok = !nc.excludes_zero();
    println!(
        "- Negativo A/A (mpmc_eventfd P=1, p99 main − aa): {:.1} µs, IC [{:.1}, {:.1}] → {}",
        us(nc.diff),
        us(nc.lo),
        us(nc.hi),
        if nc_ok {
            "PASA (incluye 0)"
        } else {
            "FALLA (excluye 0: el ruido domina)"
        }
    );
    let pc = ci(
        &values(&rows, "latency", "mpmc_eventfd", 7, "main", "p99_ns"),
        &values(&rows, "latency", "mpmc_eventfd", 1, "main", "p99_ns"),
    );
    let pc_ok = pc.excludes_zero();
    println!(
        "- Positivo (mpmc_eventfd p99 P=7 − P=1): {:.1} µs, IC [{:.1}, {:.1}] → {}",
        us(pc.diff),
        us(pc.lo),
        us(pc.hi),
        if pc_ok {
            "PASA (excluye 0)"
        } else {
            "FALLA (sin sensibilidad)"
        }
    );
    all_ok &= nc_ok && pc_ok;
    println!("\n## Hipótesis primaria (p99, msgring − mpmc_eventfd)\n");
    for &p in &[1usize, 7] {
        let c = ci(
            &values(&rows, "latency", "msgring", p, "main", "p99_ns"),
            &values(&rows, "latency", "mpmc_eventfd", p, "main", "p99_ns"),
        );
        let v = if !pc_ok {
            "no se declara: falló el control positivo".to_string()
        } else {
            let base = verdict(&c, "msgring", "mpmc_eventfd");
            if nc_ok {
                base
            } else {
                format!("{base} (NO CONFIABLE: falló A/A)")
            }
        };
        println!(
            "- P={p}: {:.1} µs, IC [{:.1}, {:.1}] → {v}",
            us(c.diff),
            us(c.lo),
            us(c.hi)
        );
    }
    println!("\n## Exploratorio (todos los pares; sin corrección por comparaciones múltiples)\n");
    println!("| métrica | P | A | B | A − B | IC 95 % | veredicto |");
    println!("|---|---|---|---|---|---|---|");
    for (phase, metric, scale, unit) in [
        ("latency", "p50_ns", 1000.0, "µs"),
        ("latency", "p99_ns", 1000.0, "µs"),
        ("latency", "p999_ns", 1000.0, "µs"),
        ("throughput", "msgs_per_s", 1.0, "msg/s"),
    ] {
        for &p in &d.producers {
            for i in 0..d.modes.len() {
                for j in (i + 1)..d.modes.len() {
                    let (a, b) = (&d.modes[i], &d.modes[j]);
                    let c = ci(
                        &values(&rows, phase, a, p, "main", metric),
                        &values(&rows, phase, b, p, "main", metric),
                    );
                    let v = if !c.excludes_zero() {
                        "IC cruza 0".to_string()
                    } else if metric == "msgs_per_s" {
                        format!("mayor: {}", if c.diff > 0.0 { a } else { b })
                    } else {
                        format!("menor: {}", if c.diff < 0.0 { a } else { b })
                    };
                    println!(
                        "| {metric} | {p} | {a} | {b} | {:.1} {unit} | [{:.1}, {:.1}] | {v} |",
                        c.diff / scale,
                        c.lo / scale,
                        c.hi / scale
                    );
                }
            }
        }
    }
    if invalid > 0 {
        println!("\nHay {invalid} corridas inválidas (excluidas del análisis).");
    }
    if all_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(3)
    }
}

fn main() -> ExitCode {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("smoke") => smoke(),
        Some("run") if a.len() == 4 => run_all(&a[2], &a[3]),
        Some("analyze") if a.len() == 4 => analyze(&a[2], &a[3]),
        _ => {
            eprintln!("uso: msgring-bench smoke | run <design.json> <runs.csv> | analyze <design.json> <runs.csv>");
            ExitCode::from(2)
        }
    }
}
