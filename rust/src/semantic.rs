//! The metadata a NIL carries (LANG.VALUES.NIL): its reason, and the text a
//! `userDeclared` reason was given.

use crate::error::NilReason;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsenceMetadata {
    pub reason: Option<NilReason>,
    /// The text a program gave `ABSENT`, for a `userDeclared` reason. Part
    /// of the value: `Value`'s equality compares it beside the reason. Held
    /// behind a *thin* pointer (`Arc<String>`, not `Arc<str>`) so the
    /// envelope stays within `value_layout_tests`' pointer-sized budget.
    pub detail: Option<std::sync::Arc<String>>,
}

impl AbsenceMetadata {
    /// The declared text, when this absence carries one.
    #[inline]
    pub fn detail_text(&self) -> Option<&str> {
        self.detail.as_deref().map(String::as_str)
    }

    pub fn literal() -> Self {
        Self {
            reason: Some(NilReason::Literal),
            detail: None,
        }
    }

    pub fn with_reasonless_unknown() -> Self {
        Self {
            reason: None,
            detail: None,
        }
    }

    pub fn with_reason(reason: NilReason) -> Self {
        Self {
            reason: Some(reason),
            detail: None,
        }
    }

    /// The absence `ABSENT` produces: reason `userDeclared`, with the
    /// program's text as its detail.
    pub fn user_declared(detail: &str) -> Self {
        Self {
            reason: Some(NilReason::UserDeclared),
            detail: Some(std::sync::Arc::new(detail.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::error::{ErrorCategory, NilReason};
    use crate::types::Value;

    #[test]
    fn nil_literal_has_diagnostic_absence_semantics() {
        let value = Value::nil();
        let absence = value
            .absence_metadata()
            .expect("literal NIL has absence metadata");

        assert!(value.is_nil());
        assert!(value.is_nil());
        // A written NIL carries a reason like every other NIL (LANG.VALUES.NIL);
        // `literal` is the one that fits — nothing failed to produce it.
        assert_eq!(absence.reason, Some(NilReason::Literal));
    }

    #[test]
    fn absence_and_diagnosis_protocol_strings_do_not_use_debug_names() {
        assert_eq!(
            ErrorCategory::ResourceLimitExceeded.as_protocol_str(),
            "resourceLimitExceeded"
        );
    }

    #[test]
    fn domain_miss_protocol_strings() {
        // LANG.FAILURE.PROJECT names the classification: "SQRT of a negative
        // rational is a well-formed domain miss".
        assert_eq!(NilReason::DomainMiss.as_protocol_str(), "domainMiss");
    }

    /// Every `NilReason` maps to a distinct, lowerCamelCase protocol string. A new
    /// variant that forgot its arm, or reused another's spelling, fails here rather
    /// than in whichever serializer happened to hit it first.
    #[test]
    fn every_nil_reason_has_a_distinct_lower_camel_protocol_string() {
        let mut seen: Vec<&str> = Vec::new();
        for reason in NilReason::ALL {
            let s = reason.as_protocol_str();
            assert!(
                s.starts_with(|c: char| c.is_ascii_lowercase())
                    && s.chars().all(|c| c.is_alphanumeric()),
                "{s} is not a lowerCamelCase protocol string"
            );
            assert!(!seen.contains(&s), "{s} is used by two reasons");
            seen.push(s);
        }
    }

    #[test]
    fn unknown_is_observed_as_a_nil() {
        // LANG.VALUES.TRUTH: UNKNOWN is NIL read in truth position, not a fourth
        // variant, so it is observed as a NIL — no truth axis.
        use crate::types::Value;
        let u = Value::nil_with_reason(NilReason::DomainMiss);
        assert_eq!(u.truth_value(), None);
        assert!(u.is_nil());
    }
    #[test]
    fn definite_truth_values_expose_truth_value_axis() {
        use crate::types::Value;
        let t = Value::from_bool(true);
        let f = Value::from_bool(false);
        assert_eq!(t.truth_value(), Some("true"));
        assert_eq!(f.truth_value(), Some("false"));
        // A plain number is not truth-valued.
        assert_eq!(Value::from_int(1).truth_value(), None);
    }
}

/// A NIL's absence as the value holds it: nothing at all, a bare reason held
/// inline, or — only when the absence carries text, as `ABSENT`'s does — a
/// box. A NIL is built per element by a Word that projects one (`'x' NUM`
/// over a column), and boxing a reason alone cost every one of them an
/// allocation.
#[derive(Debug, Clone)]
pub enum NilAbsence {
    None,
    Reason(Option<NilReason>),
    Detailed(Box<AbsenceMetadata>),
}

impl NilAbsence {
    pub fn of(absence: AbsenceMetadata) -> Self {
        match absence.detail {
            None => NilAbsence::Reason(absence.reason),
            Some(_) => NilAbsence::Detailed(Box::new(absence)),
        }
    }

    pub fn metadata(&self) -> Option<&AbsenceMetadata> {
        match self {
            NilAbsence::None => None,
            NilAbsence::Reason(reason) => Some(reason_only(*reason)),
            NilAbsence::Detailed(absence) => Some(absence),
        }
    }
}

/// The absence that is `reason` alone, shared.
fn reason_only(reason: Option<NilReason>) -> &'static AbsenceMetadata {
    const fn bare(reason: Option<NilReason>) -> AbsenceMetadata {
        AbsenceMetadata {
            reason,
            detail: None,
        }
    }
    static BARE: [AbsenceMetadata; 8] = [
        bare(None),
        bare(Some(NilReason::NotFound)),
        bare(Some(NilReason::InvalidEncoding)),
        bare(Some(NilReason::IndexOutOfBounds)),
        bare(Some(NilReason::DomainMiss)),
        bare(Some(NilReason::Literal)),
        bare(Some(NilReason::UserDeclared)),
        bare(Some(NilReason::Conflict)),
    ];
    &BARE[match reason {
        None => 0,
        Some(NilReason::NotFound) => 1,
        Some(NilReason::InvalidEncoding) => 2,
        Some(NilReason::IndexOutOfBounds) => 3,
        Some(NilReason::DomainMiss) => 4,
        Some(NilReason::Literal) => 5,
        Some(NilReason::UserDeclared) => 6,
        Some(NilReason::Conflict) => 7,
    }]
}
