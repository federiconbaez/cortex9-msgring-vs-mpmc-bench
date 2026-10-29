//! Benchmark pre-registrado: MSG_RING contra cola MPMC lock-free.

pub mod modes;
pub mod stats;

use serde_json::Value;

/// Parámetros leídos de `design.json` (el diseño pre-registrado).
#[derive(Debug, Clone)]
pub struct Design {
    pub raw: Value,
    pub modes: Vec<String>,
    pub producers: Vec<usize>,
    pub reps: usize,
    pub aa_reps: usize,
    pub lat_msgs: usize,
    pub lat_gap_ns: u64,
    pub lat_warmup: usize,
    pub tput_msgs: usize,
    pub params: modes::Params,
    pub order_seed: u64,
    pub boot_resamples: usize,
    pub boot_seed: u64,
}

impl Design {
    pub fn parse(json: &str) -> Result<Design, String> {
        let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let u = |p: &str| -> Result<u64, String> {
            v.pointer(p)
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("falta {p}"))
        };
        Ok(Design {
            modes: v["modes"]
                .as_array()
                .ok_or("modes")?
                .iter()
                .map(|m| m.as_str().unwrap().to_string())
                .collect(),
            producers: v["producers"]
                .as_array()
                .ok_or("producers")?
                .iter()
                .map(|p| p.as_u64().unwrap() as usize)
                .collect(),
            reps: u("/reps")? as usize,
            aa_reps: u("/aa_reps")? as usize,
            lat_msgs: u("/latency/msgs_per_producer")? as usize,
            lat_gap_ns: u("/latency/gap_ns")?,
            lat_warmup: u("/latency/warmup_per_producer")? as usize,
            tput_msgs: u("/throughput/msgs_per_producer")? as usize,
            params: modes::Params {
                queue_capacity: u("/queue_capacity")? as usize,
                consumer_cq_entries: u("/consumer_cq_entries")? as u32,
                producer_ring_entries: u("/producer_ring_entries")? as u32,
                consumer_cpu: u("/pin/consumer_cpu")? as usize,
                producer_cpus_start: u("/pin/producer_cpus_start")? as usize,
                pin: true,
            },
            order_seed: u("/order_seed")?,
            boot_resamples: u("/bootstrap/resamples")? as usize,
            boot_seed: u("/bootstrap/seed")?,
            raw: v,
        })
    }
}
