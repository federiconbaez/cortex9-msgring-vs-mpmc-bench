//! Los cuatro modos de paso de mensajes y una corrida.

use io_uring::{opcode, squeue, types, IoUring};
use lockfree_mpmc::MpmcQueue;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    MsgRing,
    MsgRingDefer,
    MpmcEventfd,
    MpmcSpin,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        Some(match s {
            "msgring" => Mode::MsgRing,
            "msgring_defer" => Mode::MsgRingDefer,
            "mpmc_eventfd" => Mode::MpmcEventfd,
            "mpmc_spin" => Mode::MpmcSpin,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Mode::MsgRing => "msgring",
            Mode::MsgRingDefer => "msgring_defer",
            Mode::MpmcEventfd => "mpmc_eventfd",
            Mode::MpmcSpin => "mpmc_spin",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Params {
    pub queue_capacity: usize,
    pub consumer_cq_entries: u32,
    pub producer_ring_entries: u32,
    pub consumer_cpu: usize,
    pub producer_cpus_start: usize,
    /// Fijar hilos a CPUs (falla si la CPU no existe).
    pub pin: bool,
}

#[derive(Debug, Clone)]
pub struct Spec {
    pub mode: Mode,
    pub producers: usize,
    pub msgs_per_producer: usize,
    /// 0 = sin pausa (carga de throughput).
    pub gap_ns: u64,
    pub warmup_per_producer: usize,
}

#[derive(Debug, Clone, Default)]
pub struct RunResult {
    /// Latencias en ns, sin las de calentamiento.
    pub latencies: Vec<u64>,
    pub received: usize,
    pub lost: usize,
    pub duplicated: usize,
    pub msgring_errors: u64,
    pub msgs_per_s: f64,
}

impl RunResult {
    pub fn valid(&self) -> bool {
        self.lost == 0 && self.duplicated == 0 && self.msgring_errors == 0
    }
}

#[derive(Clone, Copy)]
struct Msg {
    ts: u64,
    tag: u32,
}

fn tag(pid: usize, seq: usize) -> u32 {
    ((pid as u32) << 24) | (seq as u32 & 0x00ff_ffff)
}

pub fn pin_to(cpu: usize) -> Result<(), String> {
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_SET(cpu, &mut set);
        if libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) != 0 {
            return Err(format!(
                "sched_setaffinity cpu {cpu}: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    Ok(())
}

/// Verifica que el kernel tenga io_uring y MSG_RING. No hay fallback.
pub fn check_msgring_supported() -> Result<(), String> {
    let ring = IoUring::new(8).map_err(|e| format!("io_uring_setup: {e}"))?;
    let mut probe = io_uring::Probe::new();
    ring.submitter()
        .register_probe(&mut probe)
        .map_err(|e| format!("register_probe: {e}"))?;
    if !probe.is_supported(opcode::MsgRingData::CODE) {
        return Err("el kernel no soporta IORING_OP_MSG_RING".into());
    }
    Ok(())
}

fn now(base: &Instant) -> u64 {
    base.elapsed().as_nanos() as u64
}

fn spin_until(base: &Instant, deadline: u64) {
    while now(base) < deadline {
        std::hint::spin_loop();
    }
}

struct Collector {
    seen: Vec<Vec<bool>>,
    lat: Vec<u64>,
    received: usize,
    dup: usize,
    warmup: usize,
    last_recv: u64,
}

impl Collector {
    fn new(p: usize, n: usize, warmup: usize) -> Self {
        Collector {
            seen: vec![vec![false; n]; p],
            lat: Vec::with_capacity(p * n.saturating_sub(warmup)),
            received: 0,
            dup: 0,
            warmup,
            last_recv: 0,
        }
    }
    fn on(&mut self, ts: u64, tag: u32, now: u64) {
        let pid = (tag >> 24) as usize;
        let seq = (tag & 0x00ff_ffff) as usize;
        self.received += 1;
        self.last_recv = now;
        if self.seen[pid][seq] {
            self.dup += 1;
            return;
        }
        self.seen[pid][seq] = true;
        if seq >= self.warmup {
            self.lat.push(now.saturating_sub(ts));
        }
    }
}

/// Ejecuta una corrida. Error = el entorno no permite correrla.
pub fn run(spec: &Spec, params: &Params) -> Result<RunResult, String> {
    let p = spec.producers;
    let n = spec.msgs_per_producer;
    assert!(p < 128 && n < (1 << 24));
    let total = p * n;
    let base = Instant::now();
    let barrier = Arc::new(Barrier::new(p + 1));
    let first_send = Arc::new(AtomicU64::new(u64::MAX));
    let errors = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let queue = Arc::new(MpmcQueue::<Msg>::new(params.queue_capacity));
    let efd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC) };
    if efd < 0 {
        return Err("eventfd falló".into());
    }

    // El ring consumidor se crea en el hilo consumidor (requisito de
    // SINGLE_ISSUER/DEFER_TASKRUN); el fd se pasa a los productores.
    let (fd_tx, fd_rx) = std::sync::mpsc::channel::<Result<i32, String>>();
    let mode = spec.mode;
    let warmup = spec.warmup_per_producer;
    let pr = params.clone();
    let consumer = {
        let barrier = barrier.clone();
        let queue = queue.clone();
        let stop = stop.clone();
        std::thread::spawn(move || -> Result<Collector, String> {
            if pr.pin {
                pin_to(pr.consumer_cpu)?;
            }
            let mut col = Collector::new(p, n, warmup);
            let mut ring: Option<IoUring> = None;
            if matches!(mode, Mode::MsgRing | Mode::MsgRingDefer) {
                let mut b = IoUring::<io_uring::squeue::Entry, io_uring::cqueue::Entry>::builder();
                b.setup_cqsize(pr.consumer_cq_entries);
                if mode == Mode::MsgRingDefer {
                    b.setup_single_issuer().setup_defer_taskrun();
                }
                match b.build(8) {
                    Ok(r) => {
                        use std::os::fd::AsRawFd;
                        let _ = fd_tx.send(Ok(r.as_raw_fd()));
                        ring = Some(r);
                    }
                    Err(e) => {
                        let _ = fd_tx.send(Err(format!("io_uring_setup consumidor: {e}")));
                        return Err("ring".into());
                    }
                }
            } else {
                let _ = fd_tx.send(Ok(-1));
            }
            barrier.wait();
            match mode {
                Mode::MsgRing | Mode::MsgRingDefer => {
                    let ring = ring.as_mut().unwrap();
                    while col.received < total && !stop.load(Ordering::Relaxed) {
                        ring.submit_and_wait(1)
                            .map_err(|e| format!("submit_and_wait: {e}"))?;
                        let t = now(&base);
                        for c in ring.completion() {
                            col.on(c.user_data(), c.result() as u32, t);
                        }
                    }
                }
                Mode::MpmcEventfd => {
                    while col.received < total {
                        while let Some(m) = queue.pop() {
                            col.on(m.ts, m.tag, now(&base));
                        }
                        if col.received >= total || stop.load(Ordering::Relaxed) {
                            break;
                        }
                        let mut buf = 0u64;
                        unsafe {
                            libc::read(efd, &mut buf as *mut u64 as *mut libc::c_void, 8);
                        }
                    }
                }
                Mode::MpmcSpin => {
                    while col.received < total && !stop.load(Ordering::Relaxed) {
                        match queue.pop() {
                            Some(m) => col.on(m.ts, m.tag, now(&base)),
                            None => std::hint::spin_loop(),
                        }
                    }
                }
            }
            Ok(col)
        })
    };
    let target_fd = fd_rx.recv().map_err(|e| e.to_string())??;

    let mut producers = vec![];
    for pid in 0..p {
        let barrier = barrier.clone();
        let queue = queue.clone();
        let first_send = first_send.clone();
        let errors = errors.clone();
        let pr = params.clone();
        let gap = spec.gap_ns;
        producers.push(std::thread::spawn(move || -> Result<(), String> {
            if pr.pin {
                pin_to(pr.producer_cpus_start + pid)?;
            }
            let mut ring = match mode {
                Mode::MsgRing | Mode::MsgRingDefer => Some(
                    IoUring::new(pr.producer_ring_entries)
                        .map_err(|e| format!("io_uring_setup productor: {e}"))?,
                ),
                _ => None,
            };
            barrier.wait();
            let start = now(&base) + 1_000_000;
            for seq in 0..n {
                if gap > 0 {
                    spin_until(&base, start + seq as u64 * gap);
                }
                let ts = now(&base);
                if seq == 0 {
                    first_send.fetch_min(ts, Ordering::Relaxed);
                }
                let t = tag(pid, seq);
                match mode {
                    Mode::MsgRing | Mode::MsgRingDefer => {
                        let ring = ring.as_mut().unwrap();
                        let e = opcode::MsgRingData::new(types::Fd(target_fd), t as i32, ts, None)
                            .build()
                            .flags(squeue::Flags::SKIP_SUCCESS);
                        unsafe {
                            ring.submission().push(&e).map_err(|_| "SQ llena")?;
                        }
                        ring.submit().map_err(|e| format!("submit: {e}"))?;
                        // Con SKIP_SUCCESS, toda CQE en el ring propio es un error.
                        for _c in ring.completion() {
                            errors.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Mode::MpmcEventfd | Mode::MpmcSpin => {
                        let mut m = Msg { ts, tag: t };
                        while let Err(back) = queue.push(m) {
                            m = back;
                            std::hint::spin_loop();
                        }
                        if mode == Mode::MpmcEventfd {
                            let one = 1u64;
                            unsafe {
                                libc::write(efd, &one as *const u64 as *const libc::c_void, 8);
                            }
                        }
                    }
                }
            }
            Ok(())
        }));
    }
    let mut prod_err = None;
    for h in producers {
        if let Err(e) = h.join().unwrap() {
            prod_err = Some(e);
        }
    }
    if prod_err.is_some() || errors.load(Ordering::Relaxed) > 0 {
        // Despierta al consumidor para que no quede bloqueado.
        stop.store(true, Ordering::Relaxed);
        let one = 1u64;
        unsafe {
            libc::write(efd, &one as *const u64 as *const libc::c_void, 8);
        }
    }
    if let Some(e) = prod_err {
        // Si un productor falló, el consumidor de MSG_RING puede seguir
        // bloqueado: se reporta el error y se termina el proceso desde arriba.
        unsafe { libc::close(efd) };
        return Err(e);
    }
    let col = consumer.join().unwrap()?;
    unsafe { libc::close(efd) };
    let fs = first_send.load(Ordering::Relaxed);
    let lost = col.seen.iter().flatten().filter(|s| !**s).count();
    let mut lat = col.lat;
    lat.sort_unstable();
    Ok(RunResult {
        latencies: lat,
        received: col.received,
        lost,
        duplicated: col.dup,
        msgring_errors: errors.load(Ordering::Relaxed),
        msgs_per_s: col.received as f64 / ((col.last_recv.saturating_sub(fs)) as f64 / 1e9),
    })
}

/// Steal acumulado (ticks) de la línea `cpu` de /proc/stat.
pub fn steal_ticks() -> u64 {
    std::fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|s| {
            s.lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(8).and_then(|x| x.parse().ok()))
        })
        .unwrap_or(0)
}

pub fn loadavg1() -> f64 {
    std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|x| x.parse().ok()))
        .unwrap_or(f64::NAN)
}
