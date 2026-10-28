//! AR-VER-205: scalar truth and the Arrow-layout truth column (AR-LLR-008,
//! AR-LLR-009). The scalar connectives agree with the specification's tables;
//! the column's AND and NOT agree lane by lane with the scalar ones on every
//! pattern of up to five lanes and across word boundaries; the bytes are
//! Arrow's.

mod common;
use arare_kernel::{Truth, TruthColumn};
use common::spec_truth;

#[test]
fn ar_ver_205_scalar() {
    for (a, sa) in Truth::ALL.into_iter().zip(arare_spec::Truth::ALL) {
        assert_eq!(a.not(), spec_truth(sa.not()));
        for (b, sb) in Truth::ALL.into_iter().zip(arare_spec::Truth::ALL) {
            assert_eq!(a.and(b), spec_truth(sa.and(sb)), "{a:?} AND {b:?}");
            assert_eq!(a.or(b), spec_truth(sa.or(sb)), "{a:?} OR {b:?}");
        }
    }
    assert_eq!(Truth::of(None), Truth::Unknown);
    assert_eq!(Truth::of(Some(Truth::False)), Truth::False);
}

fn patterns(len: usize) -> Vec<Vec<Truth>> {
    let mut all = vec![vec![]];
    for _ in 0..len {
        all = all
            .into_iter()
            .flat_map(|p| {
                Truth::ALL.map(|t| {
                    let mut q = p.clone();
                    q.push(t);
                    q
                })
            })
            .collect();
    }
    all
}

#[test]
fn ar_ver_205_column_exhaustive() {
    for len in 0..=5 {
        let ps = patterns(len);
        for a in &ps {
            let ca = TruthColumn::from_lanes(a);
            let not = ca.not();
            for (i, &t) in a.iter().enumerate() {
                assert_eq!(ca.lane(i), t);
                assert_eq!(not.lane(i), t.not());
            }
            for b in &ps {
                let and = ca.and(&TruthColumn::from_lanes(b)).unwrap();
                for i in 0..len {
                    assert_eq!(and.lane(i), a[i].and(b[i]), "{a:?} AND {b:?} at {i}");
                }
            }
        }
    }
}

#[test]
fn ar_ver_205_column_across_words() {
    // A fixed linear congruential sequence: reproducible lanes over 200 lanes.
    let mut seed = 0x2545_f491u64;
    let mut next = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        Truth::ALL[(seed >> 33) as usize % 3]
    };
    let a: Vec<Truth> = (0..200).map(|_| next()).collect();
    let b: Vec<Truth> = (0..200).map(|_| next()).collect();
    let and = TruthColumn::from_lanes(&a)
        .and(&TruthColumn::from_lanes(&b))
        .unwrap();
    for i in 0..200 {
        assert_eq!(and.lane(i), a[i].and(b[i]));
    }
    assert!(TruthColumn::from_lanes(&a)
        .and(&TruthColumn::from_lanes(&b[..199]))
        .is_none());
}

#[test]
fn ar_ver_205_arrow_bytes() {
    use Truth::{False as F, True as T, Unknown as U};
    let c = TruthColumn::from_lanes(&[T, F, U, T, U, U, U, U, F]);
    let (validity, values) = (c.validity_bytes(), c.values_bytes());
    // Lane i is bit i % 8 of byte i / 8, least significant first.
    assert_eq!(&validity[..2], &[0b0000_1011, 0b0000_0001]);
    assert_eq!(&values[..2], &[0b0000_1001, 0b0000_0000]);
    // Padded to 64 bytes, with every padding bit clear.
    assert_eq!((validity.len(), values.len()), (64, 64));
    assert!(validity[2..].iter().chain(&values[2..]).all(|&b| b == 0));
    // No value bit in a NIL lane, after any operation.
    let n = c.not().and(&c).unwrap();
    for (v, x) in n.validity_bytes().iter().zip(n.values_bytes()) {
        assert_eq!(x & !v, 0);
    }
    assert_eq!(TruthColumn::from_lanes(&[]).validity_bytes().len(), 0);
    assert!(TruthColumn::from_lanes(&[]).is_empty() && c.len() == 9 && !c.is_empty());
}
