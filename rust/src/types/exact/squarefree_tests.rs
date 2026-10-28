//! The square-free split against independent answers: a naive split for
//! small radicands, and the one-word routes against the wide ones.

use super::*;

fn split(n: u128) -> (u128, u128) {
    let mut budget = u64::MAX;
    let (s, m) = squarefree_split(&BigInt::from(n), &mut budget).unwrap();
    (s.to_u128().unwrap(), m.to_u128().unwrap())
}

#[test]
fn small_radicands_split_into_square_and_square_free_parts() {
    assert_eq!(split(8), (2, 2));
    assert_eq!(split(12), (2, 3));
    assert_eq!(split(72), (6, 2));
    assert_eq!(split(30), (1, 30));
    assert_eq!(split(1), (1, 1));
    assert_eq!(split(49), (7, 1));
}

#[test]
fn large_square_factors_are_found_past_trial_division() {
    let p: u128 = 1_000_003; // prime above the trial bound
    let q: u128 = 998_244_353; // prime
    assert_eq!(split(p * p * q), (p, q));
    assert_eq!(split(p * q), (1, p * q));
    assert_eq!(split(p * p * p), (p, p));
}

#[test]
fn an_exhausted_budget_refuses_rather_than_guessing() {
    let p: u128 = 1_000_003;
    let q: u128 = 998_244_353;
    let mut budget = 10;
    assert_eq!(
        squarefree_split(&BigInt::from(p * p * q), &mut budget),
        Err(FactorBudgetExhausted)
    );
}

/// The grouped residues find every small prime the one-by-one remainder
/// did: a wide radicand built from primes at each group's edges, and every
/// radicand below a bound checked against a naive split.
#[test]
fn grouped_trial_division_agrees_with_one_remainder_per_prime() {
    let naive = |mut n: u128| {
        let (mut s, mut m, mut d) = (1u128, 1u128, 2u128);
        while d * d <= n {
            let mut e = 0;
            while n.is_multiple_of(d) {
                n /= d;
                e += 1;
            }
            s *= d.pow(e / 2);
            if e % 2 == 1 {
                m *= d;
            }
            d += 1;
        }
        (s, m * n)
    };
    for n in 1..20_000u128 {
        assert_eq!(split(n), naive(n), "{n}");
    }
    // The first and last prime of every group.
    let table = &TRIAL_TABLE.primes;
    let edges: Vec<u64> = table
        .iter()
        .enumerate()
        .filter(|&(i, prime)| {
            let before = i.checked_sub(1).map(|j| table[j].group);
            let after = table.get(i + 1).map(|next| next.group);
            before != Some(prime.group) || after != Some(prime.group)
        })
        .map(|(_, prime)| prime.p)
        .collect();
    assert!(edges.len() > 10);
    let mut wide = BigInt::from(1_000_003u64);
    let mut square = BigInt::one();
    for (i, &p) in edges.iter().enumerate() {
        let e = 1 + (i % 3) as u32;
        wide *= num_traits::pow(BigInt::from(p), e as usize);
        square *= num_traits::pow(BigInt::from(p), (e / 2) as usize);
    }
    let mut budget = u64::MAX;
    let (s, _) = squarefree_split(&wide, &mut budget).unwrap();
    assert_eq!(s, square);
}

/// The word routes answer what the wide ones do and charge the same work,
/// over primes, composites with and without small factors, squares of
/// primes and numbers near the top of each word.
#[test]
fn word_routes_match_the_wide_routes_answer_and_work() {
    let p61: u128 = (1 << 61) - 1;
    let mut odd: Vec<u128> = vec![
        16_777_259,                    // prime just above TRIAL_BOUND²
        2_147_483_647 * 1_000_003,     // two primes
        4_294_967_291 * 4_294_967_279, // near the top of the 64-bit word
        1_000_003 * 1_000_003,         // a prime's square
        p61,                           // a Mersenne prime
        u64::MAX as u128,              // 3·5·17·257·641·65537·6700417
        18_446_744_073_709_551_557,    // the largest prime below 2⁶⁴
        3_215_031_751,                 // a strong pseudoprime to 2, 3, 5, 7
        (1 << 64) + 1,                 // 274177·67280421310721, just past 2⁶⁴
        (1 << 89) - 1,                 // a Mersenne prime past 2⁶⁴
        (1 << 127) - 1,                // a Mersenne prime near the top
        u128::MAX - 158,               // the largest prime below 2¹²⁸
        u128::MAX,                     // 3·5·17·257·641·65537·274177·…
        2_147_483_647 * p61,           // two primes past 2⁶⁴
        1_000_003 * ((1 << 89) - 1),   // a small prime and a wide one
        p61 * p61,                     // a prime's square past 2⁶⁴
    ];
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for _ in 0..200 {
        let x = next();
        odd.push(u128::from((x >> (x % 40)) | (1 << 25) | 1));
    }
    // Past 2⁶⁴, a factor below 2²⁰ keeps Pollard's rho short.
    for _ in 0..100 {
        let small = u128::from(next() >> 44 | 1);
        let large = (u128::from(next()) << 36 | u128::from(next() >> 28)) | 1;
        odd.push(small * large);
    }
    for n in odd {
        let wide = BigInt::from(n);
        let (mut word_budget, mut wide_budget) = (u64::MAX, u64::MAX);
        let word_prime = is_probable_prime_word(
            &wide,
            &mut Meter {
                budget: &mut word_budget,
            },
        );
        let wide_prime = is_probable_prime_wide(
            &wide,
            &mut Meter {
                budget: &mut wide_budget,
            },
        );
        assert_eq!(word_prime, Some(wide_prime), "{n}");
        assert_eq!(word_budget, wide_budget, "{n}");
        if wide_prime == Ok(false) && exact_sqrt(&wide).is_none() {
            let word_factor = pollard_brent_word(
                &wide,
                &mut Meter {
                    budget: &mut word_budget,
                },
            );
            let wide_factor = pollard_brent_wide(
                &wide,
                &mut Meter {
                    budget: &mut wide_budget,
                },
            );
            assert_eq!(word_factor, Some(wide_factor), "{n}");
            assert_eq!(word_budget, wide_budget, "{n}");
        }
    }
}

/// Past 128 bits the word routes decline, leaving the radicand to `BigInt`.
#[test]
fn word_routes_decline_past_128_bits() {
    let wide = BigInt::from(u128::MAX) + 2u8;
    let mut budget = u64::MAX;
    let mut meter = Meter {
        budget: &mut budget,
    };
    assert!(is_probable_prime_word(&wide, &mut meter).is_none());
    assert!(pollard_brent_word(&wide, &mut meter).is_none());
}

/// The residue screen never turns a square away, and what passes it is
/// settled by the root.
#[test]
fn exact_sqrt_finds_every_square_and_only_squares() {
    for n in 0u64..100_000 {
        let root = (n as f64).sqrt() as u64;
        let expected = (0..=root + 1).find(|r| r * r == n);
        assert_eq!(
            exact_sqrt(&BigInt::from(n)),
            expected.map(BigInt::from),
            "{n}"
        );
    }
    let big = (BigInt::from(1u8) << 200usize) + 12_345u32;
    let square = &big * &big;
    assert_eq!(exact_sqrt(&square), Some(big.clone()));
    assert_eq!(exact_sqrt(&(&square + 1u8)), None);
    assert_eq!(exact_sqrt(&(&square - 1u8)), None);
}
