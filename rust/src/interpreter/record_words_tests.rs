//! Behavioral probes for the Record domain (LANG.RECORDS.STRUCTURE): the
//! eight Record Words, the two Words that now answer Records, the value
//! identity a Record carries, and the one way it lifts.

use crate::interpreter::Interpreter;
use crate::test_support::{error_of, reason, run, top};

const R: &str = "[ 'x' 'y' ] [ 1 2 ] RECORD";

#[tokio::test]
async fn record_builds_and_reads_back_in_order() {
    assert_eq!(top(R).await, "[ 'x' 'y' ] [ 1/1 2/1 ] RECORD");
    assert_eq!(top(&format!("{R} KEYS")).await, "[ 'x' 'y' ]");
    assert_eq!(top(&format!("{R} VALUES")).await, "[ 1/1 2/1 ]");
    assert_eq!(top("[ ] [ ] RECORD").await, "[ ] [ ] RECORD");
    // The two bridges compose to the identity.
    assert_eq!(
        top(&format!("{R} {R} KEYS {R} VALUES RECORD EQ")).await,
        "TRUE"
    );
}

/// A Record's keys may be any value, not only Text, and a Record nests in
/// a Vector and in another Record (LANG.RECORDS.STRUCTURE).
#[tokio::test]
async fn a_record_takes_any_key_and_nests() {
    assert_eq!(
        top("[ 1 TRUE ] [ 'one' 'yes' ] RECORD").await,
        "[ 1/1 TRUE ] [ 'one' 'yes' ] RECORD"
    );
    assert_eq!(
        top("[ 'v' 'r' ] [ 1 2 ] [ 'k' ] [ 3 ] RECORD 2 COLLECT RECORD").await,
        "[ 'v' 'r' ] [ 1/1 2/1 ] [ 'k' ] [ 3/1 ] RECORD 2 COLLECT RECORD"
    );
    assert_eq!(top("[ 'a' ] [ 1 ] RECORD 1 COLLECT LENGTH").await, "1/1");
}

#[tokio::test]
async fn record_rejects_malformed_key_vectors() {
    assert_eq!(error_of("[ 'a' 'a' ] [ 1 2 ] RECORD").await, "duplicateKey");
    assert_eq!(error_of("[ 'a' ] [ 1 2 ] RECORD").await, "shapeMismatch");
    assert_eq!(error_of("'a' [ 1 ] RECORD").await, "nonVector");
    // Operands are back on the stack after the ERROR.
    let mut interp = Interpreter::new();
    let _ = interp.execute("[ 'a' 'a' ] [ 1 2 ] RECORD").await;
    assert_eq!(interp.stack.len(), 2);
}

#[tokio::test]
async fn get_reads_a_record_by_key_and_projects_not_found() {
    assert_eq!(top(&format!("{R} 'y' GET")).await, "2/1");
    assert_eq!(
        reason(&format!("{R} 'z' GET")).await.as_deref(),
        Some("notFound")
    );
    assert_eq!(
        top(&format!("{R} 'z' GET 'S' BIND 0 S S NIL? SELECT")).await,
        "0/1"
    );
    // A Record's keys are any value, so an integer is a key like another:
    // absent here, it projects rather than being read as a position.
    assert_eq!(
        reason(&format!("{R} 0 GET")).await.as_deref(),
        Some("notFound")
    );
    // A Vector is read by index, so a String there is not one.
    assert_eq!(error_of("[ 1 2 ] 'x' GET").await, "invalidInteger");
    // Neither container: the one condition GET and PUT share.
    assert_eq!(error_of("5 'x' GET").await, "nonContainer");
    assert_eq!(error_of("5 'x' 1 PUT").await, "nonContainer");
    // A stored NIL is a value under its key: GET answers it, HAS? sees it.
    assert_eq!(top(&format!("{R} 'n' NIL PUT 'n' HAS?")).await, "TRUE");
    assert_eq!(top(&format!("{R} 'n' HAS?")).await, "FALSE");
}

#[tokio::test]
async fn put_sets_a_record_key_in_place_or_appends() {
    assert_eq!(
        top(&format!("{R} 'x' 9 PUT")).await,
        "[ 'x' 'y' ] [ 9/1 2/1 ] RECORD"
    );
    assert_eq!(top(&format!("{R} 'z' 3 PUT KEYS")).await, "[ 'x' 'y' 'z' ]");
    // The operand is a value: it is not changed by PUT, so the bound
    // Record reads unchanged after the answer.
    assert_eq!(
        top(&format!("{R} 'REC' BIND REC 'z' 3 PUT REC")).await,
        "[ 'x' 'y' 'z' ] [ 1/1 2/1 3/1 ] RECORD [ 'x' 'y' ] [ 1/1 2/1 ] RECORD"
    );
    // The key is data: an absent key passes through.
    assert_eq!(top(&format!("{R} NIL 1 PUT NIL?")).await, "TRUE");
}

#[tokio::test]
async fn without_removes_or_projects() {
    assert_eq!(
        top(&format!("{R} 'x' WITHOUT")).await,
        "[ 'y' ] [ 2/1 ] RECORD"
    );
    assert_eq!(
        reason(&format!("{R} 'z' WITHOUT")).await.as_deref(),
        Some("notFound")
    );
}

#[tokio::test]
async fn merge_is_right_biased_and_order_preserving() {
    assert_eq!(
        top(&format!("{R} [ 'y' 'z' ] [ 9 3 ] RECORD MERGE")).await,
        "[ 'x' 'y' 'z' ] [ 1/1 9/1 3/1 ] RECORD"
    );
    assert_eq!(error_of(&format!("{R} [ 1 ] MERGE")).await, "nonRecord");
}

/// LANG.VALUES.DENOTATION: the two sequences are the value.
#[tokio::test]
async fn identity_is_the_key_and_value_sequences() {
    assert_eq!(top(&format!("{R} {R} EQ")).await, "TRUE");
    assert_eq!(
        top(&format!("{R} [ 'y' 'x' ] [ 2 1 ] RECORD EQ")).await,
        "FALSE"
    );
    assert_eq!(
        top(&format!("{R} [ 'x' 'y' ] [ 1 3 ] RECORD EQ")).await,
        "FALSE"
    );
    // A Record is not the Vector of its pairs (LANG.VALUES.DISJOINT).
    assert_eq!(top("[ 'x' ] [ 1 ] RECORD [ [ 'x' 1 ] ] EQ").await, "FALSE");
    assert_eq!(
        top(&format!(
            "{R} {R} [ 'x' ] [ 1 ] RECORD 3 COLLECT UNIQUE LENGTH"
        ))
        .await,
        "2/1"
    );
}

/// Containment rule 1: arithmetic and comparison lift over the values.
#[tokio::test]
async fn arithmetic_and_comparison_lift_over_values() {
    assert_eq!(
        top(&format!("{R} 10 MUL")).await,
        "[ 'x' 'y' ] [ 10/1 20/1 ] RECORD"
    );
    assert_eq!(
        top(&format!("10 {R} SUB")).await,
        "[ 'x' 'y' ] [ 9/1 8/1 ] RECORD"
    );
    assert_eq!(
        top(&format!("{R} -1 MUL")).await,
        "[ 'x' 'y' ] [ -1/1 -2/1 ] RECORD"
    );
    assert_eq!(
        top(&format!("{R} {R} ADD")).await,
        "[ 'x' 'y' ] [ 2/1 4/1 ] RECORD"
    );
    assert_eq!(
        top(&format!("{R} 1 GT")).await,
        "[ 'x' 'y' ] [ FALSE TRUE ] RECORD"
    );
    assert_eq!(
        top(&format!("{R} 1 MAX")).await,
        "[ 'x' 'y' ] [ 1/1 2/1 ] RECORD"
    );
    // A Vector value lifts on inside the Record.
    assert_eq!(
        top("[ 'v' ] [ [ 1 2 ] ] RECORD 2 MUL").await,
        "[ 'v' ] [ [ 2/1 4/1 ] ] RECORD"
    );
    // Division by zero is a number in its lane, and the Record is kept.
    assert_eq!(
        top(&format!("{R} 0 DIV")).await,
        "[ 'x' 'y' ] [ 1/0 1/0 ] RECORD"
    );
    assert_eq!(
        error_of(&format!("{R} [ 'y' 'x' ] [ 1 2 ] RECORD ADD")).await,
        "shapeMismatch"
    );
    assert_eq!(
        top(&format!("{R} 'REC' BIND REC REC 2 MUL")).await,
        "[ 'x' 'y' ] [ 1/1 2/1 ] RECORD [ 'x' 'y' ] [ 2/1 4/1 ] RECORD"
    );
}

/// Containment rule 2: no other Word takes a Record.
#[tokio::test]
async fn no_other_family_accepts_a_record() {
    assert_eq!(error_of(&format!("{R} LENGTH")).await, "nonVector");
    assert_eq!(error_of(&format!("{R} [ 1 ADD ] MAP")).await, "nonVector");
    assert_eq!(error_of(&format!("{R} TRUE AND")).await, "nonTruthValue");
    assert_eq!(error_of(&format!("{R} CHARS")).await, "nonText");
}

#[tokio::test]
async fn tally_and_group_answer_records() {
    assert_eq!(
        top("[ 'b' 'a' 'b' ] TALLY").await,
        "[ 'b' 'a' ] [ 2/1 1/1 ] RECORD"
    );
    assert_eq!(
        top("[ 3 1 3 ] TALLY").await,
        "[ 3/1 1/1 ] [ 2/1 1/1 ] RECORD"
    );
    assert_eq!(top("[ 3 1 3 ] TALLY VALUES").await, "[ 2/1 1/1 ]");
    assert_eq!(
        top("[ 'b' 'a' 'b' 'a' ] [ 1 2 3 4 ] GROUP").await,
        "[ 'b' 'a' ] [ [ 1/1 3/1 ] [ 2/1 4/1 ] ] RECORD"
    );
    assert_eq!(
        top("[ 'a' 'b' 'a' ] [ 1 2 3 ] GROUP 'a' GET").await,
        "[ 1/1 3/1 ]"
    );
}

#[tokio::test]
async fn a_record_survives_a_block_and_the_protocol() {
    // Carried into a block as its own literal.
    assert_eq!(
        top(&format!("{R} 1 COLLECT [ 'x' GET ] MAP")).await,
        "[ 1/1 ]"
    );
    let interp = run(R).await;
    let value = interp.stack.last().cloned().expect("an answer");
    let node = crate::types::value_protocol::value_to_protocol(&value);
    assert_eq!(node.type_str, "record");
}

/// A Record's keys are one key exactly when grouping would make them one
/// (LANG.VALUES.ORDER): every NIL is one key whatever its reason, so two NIL
/// keys repeat, a NIL key is replaced by `MERGE` across reasons, and two
/// Records whose NIL keys differ only in reason lift together. `EQ` still
/// reads the reasons, as it does inside a Vector.
#[tokio::test]
async fn every_nil_is_one_record_key() {
    assert_eq!(
        error_of("'a' ABSENT 'b' ABSENT 2 COLLECT [ 1 2 ] RECORD").await,
        "duplicateKey"
    );
    assert_eq!(error_of("[ NIL NIL ] [ 1 2 ] RECORD").await, "duplicateKey");
    assert_eq!(
        error_of("[ 1 NIL ] 1 -1 SQRT 2 COLLECT 2 COLLECT [ 1 2 ] RECORD").await,
        "duplicateKey"
    );
    assert_eq!(
        top("[ 'k' NIL ] [ 1 2 ] RECORD -1 SQRT 1 COLLECT [ 9 ] RECORD MERGE").await,
        "[ 'k' NIL ] [ 1/1 9/1 ] RECORD"
    );
    // The key keeps the spelling it was given first.
    assert_eq!(
        top(
            "[ 'k' NIL ] [ 1 2 ] RECORD -1 SQRT 1 COLLECT [ 9 ] RECORD MERGE KEYS 1 GET NIL-REASON"
        )
        .await,
        "'literal'"
    );
    assert_eq!(
        top("[ NIL ] [ 1 ] RECORD -1 SQRT 1 COLLECT [ 10 ] RECORD ADD").await,
        "[ NIL ] [ 11/1 ] RECORD"
    );
    assert_eq!(
        top("[ NIL ] [ 1 ] RECORD -1 SQRT 1 COLLECT [ 1 ] RECORD EQ").await,
        "FALSE"
    );
    // GROUP's answer is a Record under the same rule.
    assert_eq!(
        top("'a' ABSENT 'b' ABSENT 2 COLLECT [ 1 2 ] GROUP VALUES").await,
        "[ [ 1/1 2/1 ] ]"
    );
}
