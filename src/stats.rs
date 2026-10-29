//! Percentiles, splitmix64, Fisher-Yates y bootstrap de diferencia de medias.

pub struct SplitMix64(pub u64);

impl SplitMix64 {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    /// Entero uniforme en [0, n) por rechazo.
    pub fn below(&mut self, n: u64) -> u64 {
        let zone = u64::MAX - (u64::MAX % n);
        loop {
            let x = self.next_u64();
            if x < zone {
                return x % n;
            }
        }
    }
}

pub fn shuffle<T>(v: &mut [T], seed: u64) {
    let mut r = SplitMix64(seed);
    for i in (1..v.len()).rev() {
        let j = r.below(i as u64 + 1) as usize;
        v.swap(i, j);
    }
}

/// Percentil por rango más cercano sobre datos ordenados (q en (0, 1]).
pub fn percentile_sorted(sorted: &[u64], q: f64) -> u64 {
    assert!(!sorted.is_empty());
    let rank = (q * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

pub fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

#[derive(Debug, Clone, Copy)]
pub struct Ci {
    pub diff: f64,
    pub lo: f64,
    pub hi: f64,
}

impl Ci {
    pub fn excludes_zero(&self) -> bool {
        self.lo > 0.0 || self.hi < 0.0
    }
}

/// IC percentil del bootstrap para mean(a) - mean(b), remuestreando cada
/// grupo por separado.
pub fn bootstrap_diff(a: &[f64], b: &[f64], resamples: usize, seed: u64, level: f64) -> Ci {
    let mut r = SplitMix64(seed);
    let mut diffs = Vec::with_capacity(resamples);
    for _ in 0..resamples {
        let ma = (0..a.len())
            .map(|_| a[r.below(a.len() as u64) as usize])
            .sum::<f64>()
            / a.len() as f64;
        let mb = (0..b.len())
            .map(|_| b[r.below(b.len() as u64) as usize])
            .sum::<f64>()
            / b.len() as f64;
        diffs.push(ma - mb);
    }
    diffs.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let alpha = (1.0 - level) / 2.0;
    let idx = |q: f64| ((q * resamples as f64).floor() as usize).min(resamples - 1);
    Ci {
        diff: mean(a) - mean(b),
        lo: diffs[idx(alpha)],
        hi: diffs[idx(1.0 - alpha)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank() {
        let v: Vec<u64> = (1..=100).collect();
        assert_eq!(percentile_sorted(&v, 0.5), 50);
        assert_eq!(percentile_sorted(&v, 0.99), 99);
        assert_eq!(percentile_sorted(&v, 0.999), 100);
        assert_eq!(percentile_sorted(&[7], 0.5), 7);
    }

    #[test]
    fn shuffle_is_deterministic_permutation() {
        let mut a: Vec<u32> = (0..50).collect();
        let mut b = a.clone();
        shuffle(&mut a, 20261029);
        shuffle(&mut b, 20261029);
        assert_eq!(a, b);
        assert_ne!(a, (0..50).collect::<Vec<_>>());
        let mut s = a.clone();
        s.sort();
        assert_eq!(s, (0..50).collect::<Vec<_>>());
    }

    #[test]
    fn bootstrap_constant_groups_give_point_ci() {
        let ci = bootstrap_diff(&[5.0; 10], &[3.0; 10], 1000, 7, 0.95);
        assert_eq!((ci.diff, ci.lo, ci.hi), (2.0, 2.0, 2.0));
        assert!(ci.excludes_zero());
    }

    #[test]
    fn bootstrap_same_distribution_includes_zero() {
        let a: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let b: Vec<f64> = (0..10).rev().map(|i| i as f64).collect();
        let ci = bootstrap_diff(&a, &b, 10_000, 7, 0.95);
        assert!(!ci.excludes_zero());
    }

    #[test]
    fn bootstrap_separated_groups_exclude_zero() {
        let a: Vec<f64> = (0..10).map(|i| 100.0 + i as f64).collect();
        let b: Vec<f64> = (0..10).map(|i| i as f64).collect();
        assert!(bootstrap_diff(&a, &b, 10_000, 7, 0.95).excludes_zero());
    }
}
