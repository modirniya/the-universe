//! Summary statistics for ensembles, and what they do and do not mean.
//!
//! Every finding in this repository is a statement about a set of universes
//! that differ only in their seed. This module turns such a set into numbers a
//! reader can weigh: not just a mean and a range, but a dispersion, a
//! confidence interval, an effect size, and — for counts like "20 of 20 seeds"
//! — an interval that says how little twenty successes actually pin down.
//!
//! # What a seed is, and is not
//!
//! A seed selects one initial condition (and, under lazy rendering, one set of
//! rendering draws) from one configuration. Twenty seeds are twenty
//! deterministic runs of the *same* rules on the *same* world size with the
//! *same* probe, differing only in which cells started alive. They are
//! independent draws of the initial condition, so an interval over seeds is an
//! honest interval over initial conditions — and over nothing else. It says
//! nothing about other sizes, other probes, other rules or other
//! approximations; those are separate axes and are varied separately where
//! the experiments sweep them.
//!
//! Twenty is also small. A 95% interval over twenty seeds is wide, and the
//! intervals here are printed so that it stays visible.
//!
//! # Determinism
//!
//! The bootstrap draws its resamples from [`crate::rng::Rng`], so a bootstrap
//! interval is as reproducible as everything else: the same values and the
//! same seed give the same interval.

use crate::rng::Rng;

/// Location, dispersion and a 95% interval for one quantity over an ensemble.
///
/// Non-finite values are dropped and `n` says how many remain, so a summary
/// that rests on fewer members than the ensemble has says so.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    pub n: usize,
    pub mean: f64,
    pub median: f64,
    /// Sample standard deviation (`n - 1`). `NaN` below two members.
    pub sd: f64,
    pub min: f64,
    pub max: f64,
    /// 95% confidence interval for the mean, from the t distribution.
    /// `NaN` below two members.
    pub ci_lo: f64,
    pub ci_hi: f64,
}

impl Summary {
    pub fn of(values: impl IntoIterator<Item = f64>) -> Summary {
        let mut v: Vec<f64> = values.into_iter().filter(|x| x.is_finite()).collect();
        let n = v.len();
        if n == 0 {
            return Summary {
                n,
                mean: f64::NAN,
                median: f64::NAN,
                sd: f64::NAN,
                min: f64::NAN,
                max: f64::NAN,
                ci_lo: f64::NAN,
                ci_hi: f64::NAN,
            };
        }
        v.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
        let mean = v.iter().sum::<f64>() / n as f64 + 0.0;
        let median = if n % 2 == 1 {
            v[n / 2]
        } else {
            (v[n / 2 - 1] + v[n / 2]) / 2.0
        };
        let sd = if n < 2 {
            f64::NAN
        } else {
            (v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1) as f64).sqrt()
        };
        let half = if n < 2 {
            f64::NAN
        } else {
            t_critical_95(n - 1) * sd / (n as f64).sqrt()
        };
        Summary {
            n,
            mean,
            median,
            sd,
            min: v[0] + 0.0,
            max: v[n - 1] + 0.0,
            ci_lo: mean - half,
            ci_hi: mean + half,
        }
    }

    /// Standard error of the mean.
    pub fn se(&self) -> f64 {
        self.sd / (self.n as f64).sqrt()
    }
}

/// Two-sided 95% critical value of Student's t with `df` degrees of freedom.
///
/// A table rather than a quantile function: the only degrees of freedom this
/// project ever needs are small, and a table is exact where it is tabulated
/// and reproducible everywhere.
pub fn t_critical_95(df: usize) -> f64 {
    const TABLE: [(usize, f64); 24] = [
        (1, 12.706),
        (2, 4.303),
        (3, 3.182),
        (4, 2.776),
        (5, 2.571),
        (6, 2.447),
        (7, 2.365),
        (8, 2.306),
        (9, 2.262),
        (10, 2.228),
        (11, 2.201),
        (12, 2.179),
        (13, 2.160),
        (14, 2.145),
        (15, 2.131),
        (16, 2.120),
        (17, 2.110),
        (18, 2.101),
        (19, 2.093),
        (20, 2.086),
        (25, 2.060),
        (30, 2.042),
        (60, 2.000),
        (120, 1.980),
    ];
    if df == 0 {
        return f64::NAN;
    }
    for pair in TABLE.windows(2) {
        let (d0, t0) = pair[0];
        let (d1, t1) = pair[1];
        if df == d0 {
            return t0;
        }
        if df < d1 {
            // Linear interpolation between tabulated rows.
            return t0 + (t1 - t0) * (df - d0) as f64 / (d1 - d0) as f64;
        }
    }
    1.960
}

/// Wilson score interval for a proportion `k / n`, at 95%.
///
/// "20 of 20 seeds" sounds decisive. Its Wilson interval is [0.84, 1.00]: twenty
/// successes are consistent with a true rate as low as 84%. Every count of
/// seeds in a report is printed with this beside it.
pub fn wilson(k: usize, n: usize) -> (f64, f64) {
    if n == 0 {
        return (f64::NAN, f64::NAN);
    }
    let z = 1.96;
    let (k, n) = (k as f64, n as f64);
    let p = k / n;
    let z2 = z * z;
    let denom = 1.0 + z2 / n;
    let centre = (p + z2 / (2.0 * n)) / denom;
    let half = z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / denom;
    ((centre - half).max(0.0), (centre + half).min(1.0))
}

/// Cohen's d between two samples: the difference of means in units of the
/// pooled standard deviation. `NaN` when either sample has fewer than two
/// members or both have no spread.
pub fn cohens_d(a: &[f64], b: &[f64]) -> f64 {
    let (sa, sb) = (
        Summary::of(a.iter().copied()),
        Summary::of(b.iter().copied()),
    );
    if sa.n < 2 || sb.n < 2 {
        return f64::NAN;
    }
    let pooled = (((sa.n - 1) as f64 * sa.sd * sa.sd + (sb.n - 1) as f64 * sb.sd * sb.sd)
        / (sa.n + sb.n - 2) as f64)
        .sqrt();
    if pooled == 0.0 {
        return f64::NAN;
    }
    (sa.mean - sb.mean) / pooled
}

/// Summary of the paired differences `a[i] - b[i]`.
///
/// The right comparison when every seed produced both an `a` and a `b`: a
/// limit's divergence and the null's divergence *in the same universe*.
pub fn paired(a: &[f64], b: &[f64]) -> Summary {
    Summary::of(a.iter().zip(b).map(|(x, y)| x - y))
}

/// Where `x` falls in a null distribution, in standard deviations.
///
/// `NaN` if the null has no spread or fewer than two members.
pub fn z_score(x: f64, null: &[f64]) -> f64 {
    let s = Summary::of(null.iter().copied());
    if s.n < 2 || !(s.sd > 0.0) {
        return f64::NAN;
    }
    (x - s.mean) / s.sd
}

/// Share of the null at or above `x`: a one-sided empirical tail, with the
/// `+1` correction so that a value above every null member reports
/// `1 / (n + 1)` rather than zero.
pub fn tail(x: f64, null: &[f64]) -> f64 {
    let n = null.iter().filter(|v| v.is_finite()).count();
    if n == 0 {
        return f64::NAN;
    }
    let above = null.iter().filter(|v| v.is_finite() && **v >= x).count();
    (above + 1) as f64 / (n + 1) as f64
}

/// Deterministic percentile bootstrap of `stat` over `values`.
///
/// Returns the 2.5th and 97.5th percentiles of `stat` over `resamples`
/// resamples drawn with replacement through the project's own generator.
pub fn bootstrap_ci(
    values: &[f64],
    seed: u64,
    resamples: usize,
    stat: impl Fn(&[f64]) -> f64,
) -> (f64, f64) {
    let v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.len() < 2 || resamples == 0 {
        return (f64::NAN, f64::NAN);
    }
    let mut rng = Rng::derive(seed, 0xB007_57A9, v.len() as u64, resamples as u64);
    let mut draws: Vec<f64> = (0..resamples)
        .map(|_| {
            let sample: Vec<f64> = (0..v.len())
                .map(|_| v[(rng.next_u64() % v.len() as u64) as usize])
                .collect();
            stat(&sample)
        })
        .filter(|x| x.is_finite())
        .collect();
    if draws.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    draws.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    (percentile(&draws, 0.025), percentile(&draws, 0.975))
}

/// Linear-interpolated percentile of a sorted slice.
pub fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let pos = q.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
    }
}

/// Pearson correlation. `NaN` when either series is constant or shorter than two.
pub fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n < 2 {
        return f64::NAN;
    }
    let (a, b) = (&a[..n], &b[..n]);
    let ma = a.iter().sum::<f64>() / n as f64;
    let mb = b.iter().sum::<f64>() / n as f64;
    let (mut num, mut da, mut db) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (x, y) = (a[i] - ma, b[i] - mb);
        num += x * y;
        da += x * x;
        db += y * y;
    }
    if da == 0.0 || db == 0.0 {
        return f64::NAN;
    }
    num / (da * db).sqrt()
}

/// Shannon entropy in bits of a discrete distribution given as counts.
pub fn entropy_bits(counts: &[usize]) -> f64 {
    let total: usize = counts.iter().sum();
    if total == 0 {
        return 0.0;
    }
    counts
        .iter()
        .filter(|c| **c > 0)
        .map(|c| {
            let p = *c as f64 / total as f64;
            -p * p.log2()
        })
        .sum::<f64>()
        + 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_summary_of_a_known_sample() {
        let s = Summary::of([2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        assert_eq!(s.n, 8);
        assert!((s.mean - 5.0).abs() < 1e-12);
        assert!((s.median - 4.5).abs() < 1e-12);
        // Sample sd of this classic set is sqrt(32/7).
        assert!((s.sd - (32.0f64 / 7.0).sqrt()).abs() < 1e-12);
        assert_eq!((s.min, s.max), (2.0, 9.0));
        assert!(s.ci_lo < s.mean && s.mean < s.ci_hi);
    }

    #[test]
    fn non_finite_members_are_dropped_and_counted_out() {
        let s = Summary::of([1.0, f64::NAN, 3.0, f64::INFINITY]);
        assert_eq!(s.n, 2);
        assert_eq!(s.mean, 2.0);
    }

    #[test]
    fn a_single_member_has_no_interval() {
        let s = Summary::of([4.0]);
        assert_eq!(s.n, 1);
        assert!(s.sd.is_nan() && s.ci_lo.is_nan());
        assert_eq!(s.median, 4.0);
    }

    #[test]
    fn t_table_is_monotone_and_ends_at_the_normal_value() {
        let mut prev = f64::INFINITY;
        for df in 1..200 {
            let t = t_critical_95(df);
            assert!(t <= prev + 1e-12, "df {df}: {t} > {prev}");
            prev = t;
        }
        assert_eq!(t_critical_95(19), 2.093);
        assert_eq!(t_critical_95(1000), 1.960);
    }

    #[test]
    fn twenty_of_twenty_is_not_certainty() {
        let (lo, hi) = wilson(20, 20);
        assert!(lo > 0.83 && lo < 0.85, "{lo}");
        assert_eq!(hi, 1.0);
        let (lo, hi) = wilson(0, 20);
        assert_eq!(lo, 0.0);
        assert!(hi > 0.15 && hi < 0.17, "{hi}");
        let (lo, hi) = wilson(10, 20);
        assert!(lo < 0.5 && hi > 0.5);
    }

    #[test]
    fn cohens_d_is_in_pooled_sd_units() {
        let a = [1.0, 2.0, 3.0];
        let b = [3.0, 4.0, 5.0];
        assert!((cohens_d(&a, &b) + 2.0).abs() < 1e-12);
        assert!(cohens_d(&[1.0], &b).is_nan());
    }

    #[test]
    fn paired_differences_are_elementwise() {
        let s = paired(&[3.0, 5.0, 7.0], &[1.0, 1.0, 1.0]);
        assert_eq!(s.n, 3);
        assert_eq!(s.mean, 4.0);
    }

    #[test]
    fn z_and_tail_agree_on_a_simple_null() {
        let null: Vec<f64> = (0..100).map(|i| i as f64).collect();
        assert!(z_score(99.0, &null) > 1.6);
        assert!((tail(99.0, &null) - 2.0 / 101.0).abs() < 1e-12);
        assert!((tail(0.0, &null) - 1.0).abs() < 1e-12);
        assert!(z_score(1.0, &[1.0, 1.0]).is_nan());
    }

    #[test]
    fn the_bootstrap_is_deterministic_and_brackets_the_mean() {
        let v: Vec<f64> = (0..30).map(|i| (i % 7) as f64).collect();
        let mean = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;
        let a = bootstrap_ci(&v, 7, 500, mean);
        let b = bootstrap_ci(&v, 7, 500, mean);
        assert_eq!(a, b);
        let m = mean(&v);
        assert!(a.0 < m && m < a.1, "{a:?} should bracket {m}");
        assert_ne!(
            a,
            bootstrap_ci(&v, 8, 500, mean),
            "a different seed, a different draw"
        );
    }

    #[test]
    fn percentiles_interpolate() {
        let s = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile(&s, 0.0), 1.0);
        assert_eq!(percentile(&s, 1.0), 4.0);
        assert_eq!(percentile(&s, 0.5), 2.5);
    }

    #[test]
    fn correlation_and_entropy_basics() {
        let a = [0.1, 0.5, 0.2, 0.9, 0.4];
        assert!((correlation(&a, &a) - 1.0).abs() < 1e-12);
        assert!(correlation(&[1.0, 1.0, 1.0], &a).is_nan());
        assert_eq!(entropy_bits(&[4, 4]), 1.0);
        assert_eq!(entropy_bits(&[8, 0]), 0.0);
        assert!((entropy_bits(&[1, 1, 1, 1]) - 2.0).abs() < 1e-12);
    }
}
