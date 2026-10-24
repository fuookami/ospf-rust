//! # 素数算法 / Prime Number Algorithm
//!
//! 提供素数判断、素数列表生成等功能。
//! Provides prime number checking, prime list generation, and related functions.

use std::sync::{Mutex, OnceLock};
use std::vec::Vec;

/// 素数缓存结构
/// Prime number cache structure
///
/// 使用埃拉托斯特尼筛法缓存素数，支持动态扩展。
/// Uses Sieve of Eratosthenes to cache primes, supports dynamic extension.
pub struct PrimeCache {
    current: usize,
    is_prime: Vec<bool>,
    primes: Vec<usize>,
}

impl PrimeCache {
    /// 创建新的素数缓存
    /// Create a new prime cache
    fn new() -> Self {
        let mut cache = Self {
            current: 0,
            is_prime: Vec::new(),
            primes: Vec::new(),
        };
        cache.sieve(1000);
        cache
    }

    /// 扩展筛法范围
    /// Extend the sieve range
    fn extend_sieve(&mut self, new_limit: usize) {
        if new_limit <= self.current {
            return;
        }

        let old_limit = self.current;
        let new_size = new_limit + 1;
        let sqrt_limit = (new_limit as f64).sqrt() as usize;

        // 扩展 is_prime 数组
        // Extend is_prime array
        self.is_prime.resize(new_size, true);

        if old_limit == 0 {
            // new_limit >= 0 恒为真，因为 new_limit 是 usize
            // new_limit >= 0 is always true since new_limit is usize
            self.is_prime[0] = false;
            if new_limit >= 1 {
                self.is_prime[1] = false;
            }
        }

        self.current = new_limit;

        // 用已有的素数标记新范围内的合数
        // Mark composites in new range using existing primes
        for &p in &self.primes {
            if p > sqrt_limit {
                break;
            }

            let start = if p * p > old_limit + 1 {
                p * p
            } else {
                ((old_limit + 1 + p - 1) / p) * p
            };

            for j in (start..=new_limit).step_by(p) {
                self.is_prime[j] = false;
            }
        }

        // 处理新范围内的素数
        // Process primes in new range
        let start = if old_limit < 2 { 2 } else { old_limit + 1 };
        for i in start..=new_limit {
            if self.is_prime[i] {
                self.primes.push(i);

                if i <= sqrt_limit {
                    let start_multiple = i * i.max(start);
                    for j in (start_multiple..=new_limit).step_by(i) {
                        self.is_prime[j] = false;
                    }
                }
            }
        }
    }

    /// 执行筛法
    /// Perform the sieve
    fn sieve(&mut self, limit: usize) {
        if limit <= self.current {
            return;
        }

        self.is_prime = vec![true; limit + 1];
        // limit >= 0 恒为真，因为 limit 是 usize
        // limit >= 0 is always true since limit is usize
        self.is_prime[0] = false;
        if limit >= 1 {
            self.is_prime[1] = false;
        }

        let sqrt_limit = (limit as f64).sqrt() as usize;

        for i in 2..=sqrt_limit {
            if self.is_prime[i] {
                for j in ((i * i)..=limit).step_by(i) {
                    self.is_prime[j] = false;
                }
            }
        }

        self.primes.clear();
        for i in 2..=limit {
            if self.is_prime[i] {
                self.primes.push(i);
            }
        }

        self.current = limit;
    }

    /// 获取小于等于 limit 的所有素数
    /// Get all primes up to limit
    pub fn get_primes(&mut self, limit: usize) -> Vec<usize> {
        if limit > self.current {
            self.extend_sieve(limit);
        }
        self.primes
            .iter()
            .filter(|&&p| p <= limit)
            .copied()
            .collect()
    }

    /// 判断一个数是否为素数
    /// Check if a number is prime
    pub fn is_prime(&mut self, num: usize) -> bool {
        if num <= 1 {
            return false;
        }

        if num > self.current {
            if num <= 1_000_000 {
                self.extend_sieve(num);
                return self.is_prime[num];
            } else {
                return self.is_prime_quick_check(num);
            }
        }

        self.is_prime[num]
    }

    /// 不扩展缓存的确定性大数检查 / Deterministic large-number check without growing the cache.
    fn is_prime_quick_check(&self, n: usize) -> bool {
        is_prime_deterministic(n as u64)
    }
}

/// 全局素数缓存
/// Global prime cache
static PRIME_CACHE: OnceLock<Mutex<PrimeCache>> = OnceLock::new();

/// 获取素数缓存的引用
/// Get reference to prime cache
fn get_cache() -> &'static Mutex<PrimeCache> {
    PRIME_CACHE.get_or_init(|| Mutex::new(PrimeCache::new()))
}

/// 判断一个数是否为素数
/// Check if a number is prime
///
/// # 参数 / Parameters
///
/// * `num` - 要检查的数 / Number to check
///
/// # 返回值 / Returns
///
/// 如果是素数返回 `true`，否则返回 `false`
/// Returns `true` if prime, `false` otherwise
///
/// # 示例 / Example
///
/// ```
/// use ospf_rust_math::ordinary::is_prime;
///
/// assert!(is_prime(7));
/// assert!(!is_prime(4));
/// ```
pub fn is_prime(num: usize) -> bool {
    {
        let mut cache = get_cache().lock()
            .expect("prime cache mutex poisoned / 素数缓存互斥锁已中毒");
        if num <= cache.current || num <= 1_000_000 { return cache.is_prime(num); }
    }
    // 缓存查询结束即释放锁，模幂计算不阻塞其他筛查询。 / Release the cache lock before modular powers so other sieve queries can proceed.
    is_prime_deterministic(num as u64)
}

/// 获取小于等于 limit 的所有素数
/// Get all primes up to limit
///
/// # 参数 / Parameters
///
/// * `limit` - 上限值 / Upper limit
///
/// # 返回值 / Returns
///
/// 素数列表 / List of primes
///
/// # 示例 / Example
///
/// ```
/// use ospf_rust_math::ordinary::get_primes;
///
/// let primes = get_primes(10);
/// assert_eq!(primes, vec![2, 3, 5, 7]);
/// ```
pub fn get_primes(limit: usize) -> Vec<usize> {
    get_cache()
        .lock()
        .expect("prime cache mutex poisoned / 素数缓存互斥锁已中毒")
        .get_primes(limit)
}

/// 判断一个 u64 数是否为素数
/// Check if a u64 number is prime
pub fn is_prime_u64(num: u64) -> bool {
    match usize::try_from(num) {
        Ok(num) => is_prime(num),
        Err(_) => is_prime_deterministic(num),
    }
}

fn modular_power(
    mut base: u64,
    mut exponent: u64,
    modulus: u64,
) -> u64 {
    let mut result = 1 % modulus;
    base %= modulus;
    // 32位模数的剩余数乘积可由u64精确容纳，避免不必要的宽除法。 / Residue products for 32-bit moduli fit exactly in u64, avoiding unnecessary wide division.
    if modulus <= u32::MAX as u64 {
        while exponent != 0 {
            if exponent & 1 != 0 { result = result * base % modulus; }
            base = base * base % modulus;
            exponent >>= 1;
        }
        return result;
    }
    while exponent != 0 {
        if exponent & 1 != 0 { result = (result as u128 * base as u128 % modulus as u128) as u64; }
        base = (base as u128 * base as u128 % modulus as u128) as u64;
        exponent >>= 1;
    }
    result
}

fn is_prime_deterministic(n: u64) -> bool {
    if n < 2 { return false; }
    for prime in [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        if n % prime == 0 { return n == prime; }
    }
    let powers = (n - 1).trailing_zeros();
    let odd = (n - 1) >> powers;
    // 三见证覆盖u32，七见证覆盖u64；宽乘积不溢出，不是概率素性判断。 / Three witnesses cover u32 and seven cover u64; wide products avoid overflow, making this deterministic rather than probabilistic.
    let witnesses: &[u64] = if n <= u32::MAX as u64 { &[2, 7, 61] }
        else { &[2, 325, 9375, 28178, 450775, 9780504, 1795265022] };
    for &witness in witnesses {
        let base = witness % n;
        if base == 0 { continue; }
        let mut value = modular_power(base, odd, n);
        if value == 1 || value == n - 1 { continue; }
        let mut passed = false;
        for _ in 1..powers {
            value = (value as u128 * value as u128 % n as u128) as u64;
            if value == n - 1 { passed = true; break; }
        }
        if !passed { return false; }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trial_prime(n: u64) -> bool {
        if n < 2 { return false; }
        let mut divisor = 2;
        while divisor <= n / divisor {
            if n % divisor == 0 { return false; }
            divisor += 1;
        }
        true
    }

    #[test]
    fn deterministic_witnesses_match_sieve_trial_and_full_width_cases() {
        let mut sieve = vec![true; 100_001];
        sieve[0] = false;
        sieve[1] = false;
        for divisor in 2..=316 {
            if sieve[divisor] {
                for multiple in (divisor * divisor..sieve.len()).step_by(divisor) { sieve[multiple] = false; }
            }
        }
        for (number, expected) in sieve.into_iter().enumerate() {
            assert_eq!(is_prime_deterministic(number as u64), expected, "{number}");
        }
        let mut random = 0x3141_5926_5358_9793u64;
        for _ in 0..512 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let number = random % 10_000_000_000;
            assert_eq!(is_prime_u64(number), trial_prime(number), "{number}");
        }
        for number in [2, 37, 41, 1_000_000_007, 4_294_967_291, 2_305_843_009_213_693_951, 18_446_744_073_709_551_557] {
            assert!(is_prime_u64(number), "{number}");
        }
        for number in [0, 1, 4, 561, 1105, 1729, 2465, 2821, 6601, 1_373_653, 9_080_191, 3_215_031_751, 4_759_123_141, 341_550_071_728_321,
            3_825_123_056_546_413_051, 4_294_967_291u64 * 4_294_967_291, u64::MAX - 1, u64::MAX]
        {
            assert!(!is_prime_u64(number), "{number}");
        }
    }

    #[test]
    fn modular_powers_match_big_integer_oracle_without_overflow() {
        use num_bigint::BigUint;
        use num_traits::ToPrimitive;
        for modulus in [1, 2, 41, 4_294_967_291, 4_294_967_295, 4_294_967_296, u64::MAX - 58, u64::MAX] {
            for base in [0, 1, 2, 450775, u64::MAX - 1, u64::MAX] {
                for exponent in [0, 1, 2, 63, u64::MAX] {
                    let expected = BigUint::from(base).modpow(&BigUint::from(exponent), &BigUint::from(modulus)).to_u64().unwrap();
                    assert_eq!(modular_power(base, exponent, modulus), expected);
                }
            }
        }
    }

    #[test]
    fn sieve_growth_cached_queries_and_concurrent_large_checks_remain_consistent() {
        let mut cache = PrimeCache::new();
        let primes = cache.get_primes(1_000_100);
        assert_eq!(primes.last(), Some(&1_000_099));
        let current = cache.current;
        for number in [0, 1, 2, 997, 1009, 1_000_003, 1_000_099, 1_000_101] {
            assert_eq!(cache.is_prime(number), trial_prime(number as u64));
        }
        assert_eq!(cache.current, current);
        std::thread::scope(|scope| {
            let workers = (0..4).map(|worker| scope.spawn(move || {
                for iteration in 0..64 {
                    assert!(is_prime_u64(18_446_744_073_709_551_557));
                    assert!(!is_prime_u64(3_825_123_056_546_413_051));
                    let limit = 1000 + worker * 100 + iteration;
                    assert!(get_primes(limit).into_iter().all(|number| trial_prime(number as u64)));
                }
            })).collect::<Vec<_>>();
            for worker in workers { worker.join().unwrap(); }
        });
    }

    #[test]
    #[ignore = "手动 release 性能测量 / Manual release performance measurement"]
    fn performance_b09_prime_queries() {
        use std::hint::black_box;
        use std::time::Instant;
        for number in [1_000_003, 1_000_000_007, 1_000_000_000_039u64] {
            assert!(is_prime_u64(number));
            for sample in 0..5 {
                let start = Instant::now();
                for _ in 0..10 { assert!(black_box(is_prime_u64(black_box(number)))); }
                println!("prime_queries number={number} repetitions=10 sample={sample} elapsed={:?}", start.elapsed());
            }
        }
    }

    #[test]
    fn test_is_prime() {
        assert!(!is_prime(0));
        assert!(!is_prime(1));
        assert!(is_prime(2));
        assert!(is_prime(3));
        assert!(!is_prime(4));
        assert!(is_prime(5));
        assert!(!is_prime(6));
        assert!(is_prime(7));
        assert!(!is_prime(8));
        assert!(!is_prime(9));
        assert!(!is_prime(10));
        assert!(is_prime(11));
        assert!(is_prime(13));
        assert!(is_prime(17));
        assert!(is_prime(19));
        assert!(is_prime(23));
        assert!(is_prime(97));
        assert!(!is_prime(100));
    }

    #[test]
    fn test_get_primes() {
        let primes = get_primes(10);
        assert_eq!(primes, vec![2, 3, 5, 7]);

        let primes = get_primes(20);
        assert_eq!(primes, vec![2, 3, 5, 7, 11, 13, 17, 19]);
    }

    #[test]
    fn test_large_prime() {
        assert!(is_prime(104729)); // 第 10000 个素数
        assert!(!is_prime(104730));
    }
}
