//! A rational held in two machine words is encoded in place
//! (`encode_rational`'s `Small` arm); the bytes must be the ones the `BigInt`
//! spelling gives the same value's lowest terms, of either sign.

use super::*;
use num_integer::Integer;
use num_traits::Zero;
use proptest::prelude::*;

fn encoded_by_bigint(n: i64, d: i64) -> Vec<u8> {
    let (mut n, mut d) = (BigInt::from(n), BigInt::from(d));
    let g = n.gcd(&d);
    if !g.is_zero() {
        n /= &g;
        d /= &g;
    }
    if d.sign() == Sign::Minus {
        n = -n;
        d = -d;
    }
    let mut bytes = vec![b'Q'];
    write_sint(&mut bytes, &n);
    write_sint(&mut bytes, &d);
    bytes
}

fn encoded(n: i64, d: i64) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode_rational(&mut bytes, &Fraction::new(BigInt::from(n), BigInt::from(d)));
    bytes
}

fn half() -> impl Strategy<Value = i64> {
    prop_oneof![
        4 => any::<i64>(),
        3 => -1000i64..1000,
        1 => Just(i64::MIN),
        1 => Just(i64::MAX),
        1 => Just(0i64),
        1 => Just(1i64),
        1 => Just(-1i64),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20000))]

    #[test]
    fn a_small_pair_encodes_as_the_bigint_route_does(
        n in half(),
        d in half().prop_map(|d| if d == 0 { 7 } else { d }),
    ) {
        prop_assert_eq!(encoded(n, d), encoded_by_bigint(n, d));
    }
}

#[test]
fn the_edge_pairs_encode_as_the_bigint_route_does() {
    for (n, d) in [
        (i64::MIN, -1),
        (i64::MIN, i64::MIN),
        (i64::MIN, 1),
        (0, -5),
        (6, -4),
        (-6, -4),
        (i64::MAX, i64::MIN),
    ] {
        assert_eq!(encoded(n, d), encoded_by_bigint(n, d), "{n}/{d}");
    }
}
