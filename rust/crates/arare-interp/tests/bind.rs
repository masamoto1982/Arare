//! AR-VER-228: BIND and names in the interpreter (AR-LLR-028, refining
//! AR-HLR-032): a name pushes the value bound to it, as often as it is
//! written; binding it again replaces the value; a block reads the names of
//! the code around it and its own bindings end with it; a block of names
//! destructures a vector; and every way BIND refuses malformed use.

use arare_front::Word;
use arare_interp::{run, Error, ErrorKind as K, RunError, Value};

fn shown(src: &str) -> String {
    match run(src) {
        Ok(stack) => stack
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(" "),
        Err(e) => panic!("{src}: {e}"),
    }
}

fn error(src: &str) -> Error {
    match run(src) {
        Err(RunError::Interp(e)) => e,
        other => panic!("{src}: expected an error, got {other:?}"),
    }
}

#[test]
fn ar_ver_228_bind() {
    // Both operands are consumed and nothing is pushed.
    assert_eq!(shown("5 'N' BIND"), "");
    assert_eq!(shown("5 'N' BIND N N ADD"), "10/1");
    // Binding again replaces the value.
    assert_eq!(shown("5 'N' BIND 6 'N' BIND N"), "6/1");
    // Any value: a vector, a truth value, NIL, a quote (which stays code).
    assert_eq!(shown("[ 1 2 ] 'V' BIND V V ADD"), "[ 2/1 4/1 ]");
    assert_eq!(shown("TRUE 'T' BIND T NOT"), "FALSE");
    assert_eq!(shown("NIL 'Z' BIND Z"), "NIL(literal)");
    assert_eq!(shown("[ 2 MUL ] 'F' BIND [ 1 2 ] F MAP"), "[ 2/1 4/1 ]");
    // Several names take a vector of the same length, lane by lane.
    assert_eq!(shown("[ 1 2 ] [ 'A' 'B' ] BIND B A SUB"), "1/1");
    assert_eq!(shown("[ 7 ] [ 'A' ] BIND A"), "7/1");
    // Text is a value only BIND reads; EQ compares it as a name.
    assert_eq!(shown("'A' 'A' EQ 'A' 'B' EQ"), "TRUE FALSE");
    assert_eq!(Value::Text("A".into()).to_string(), "'A'");
}

#[test]
fn ar_ver_228_scopes() {
    // A block reads the names of the code around it.
    assert_eq!(shown("10 'K' BIND [ 1 2 ] [ K MUL ] MAP"), "[ 10/1 20/1 ]");
    assert_eq!(
        shown("[ 1 2 3 ] 0 [ 'I' BIND 'X' BIND X I ADD ] FOLD"),
        "6/1"
    );
    // A binding made in a block ends with it, and one made in a block shadows
    // the outer one there only.
    assert_eq!(
        shown("5 'N' BIND [ 1 ] [ 'N' BIND N ] MAP N"),
        "[ 1/1 ] 5/1"
    );
    assert_eq!(
        error("[ 1 ] [ 'K' BIND K ] MAP K"),
        Error {
            word: None,
            kind: K::Unbound("K".into())
        }
    );
    // An error in a block is the Word's that ran it, bindings or not.
    let e = error("[ 1 ] [ 'K' BIND K TRUE ADD ] MAP");
    assert_eq!((e.word, e.kind), (Some(Word::Map), K::Type("a number")));
    // A bracket read as data holds only literals; a name in it is refused,
    // not read, as Ajisai keeps it a name (AR-HLR-021).
    assert_eq!(
        error("3 'C' BIND [ C C ] SUM"),
        Error {
            word: Some(Word::Sum),
            kind: K::NotLiteral("C".into())
        }
    );
    // A FILTER's predicate reads them too.
    assert_eq!(shown("2 'T' BIND [ 1 2 3 ] [ T LT ] FILTER"), "[ 1/1 ]");
}

#[test]
fn ar_ver_228_refusals() {
    let kind = |src: &str| error(src).kind;
    assert_eq!(kind("'N' BIND"), K::StackUnderflow { needed: 2, had: 1 });
    assert_eq!(error("'N' BIND").word, Some(Word::Bind));
    assert_eq!(kind("N 'N' BIND"), K::Unbound("N".into()));
    assert_eq!(error("N 'N'").to_string(), "N is not bound here");
    assert_eq!(
        kind("'A' 'B' BIND"),
        K::BadOperand("a value to bind that is not a name")
    );
    assert_eq!(
        kind("5 6 BIND"),
        K::BadOperand("a name, or a block of names")
    );
    assert_eq!(
        kind("5 [ 1 ] BIND"),
        K::BadOperand("a name, or a block of names")
    );
    assert_eq!(kind("[ 1 2 ] [ 'A' ] BIND"), K::Shape { left: 2, right: 1 });
    assert_eq!(kind("5 [ 'A' ] BIND"), K::Type("a vector"));
    // A block that reads its own name, directly or through other bindings.
    assert_eq!(kind("[ X ] 'X' BIND"), K::SelfReference("X".into()));
    assert_eq!(kind("[ [ X ] ] 'X' BIND"), K::SelfReference("X".into()));
    assert_eq!(
        kind("[ Y ] 'X' BIND [ X ] 'Y' BIND"),
        K::SelfReference("Y".into())
    );
    assert_eq!(
        error("[ X ] 'X' BIND").to_string(),
        "BIND: the block bound to X would read X itself"
    );
    // Reading other names, even repeatedly, is fine.
    assert_eq!(
        shown("[ 1 ] 'A' BIND [ A A ] 'B' BIND [ B A ] 'C' BIND C 'D' BIND 7"),
        "7/1"
    );
    // Text is no number.
    assert_eq!(kind("'A' 1 ADD"), K::Type("a number"));
}
