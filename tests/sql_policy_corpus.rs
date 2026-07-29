#![allow(clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use dmlpact::integrity::sha256_bytes;
use dmlpact::sql::analyze_sql;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema_version: String,
    license: String,
    labeling_methodology: String,
    requirements: Requirements,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Requirements {
    minimum_cases: usize,
    required_supported_acceptance: f64,
    required_unsupported_rejection: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    category: String,
    sql: String,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    decision: String,
    statement_kind: Option<String>,
    requested_table: Option<String>,
    inserted_rows: Option<u64>,
    error_code: Option<String>,
}

#[derive(Default)]
struct CategoryMetrics {
    cases: usize,
    exact_matches: usize,
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sql-policy/v0.1")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read JSON fixture")).expect("parse JSON fixture")
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        1.0
    } else {
        let numerator = u32::try_from(numerator).expect("corpus count fits u32");
        let denominator = u32::try_from(denominator).expect("corpus count fits u32");
        f64::from(numerator) / f64::from(denominator)
    }
}

#[test]
fn published_sql_policy_metrics_are_reproducible() {
    let root = fixture_root();
    let corpus_bytes = fs::read(root.join("corpus.json")).expect("read corpus");
    let corpus: Corpus = serde_json::from_slice(&corpus_bytes).expect("corpus shape");
    assert_eq!(corpus.schema_version, "dmlpact.sql-policy-corpus/v0.1");
    assert_eq!(corpus.license, "MIT");
    assert!(!corpus.labeling_methodology.is_empty());
    assert!(corpus.cases.len() >= corpus.requirements.minimum_cases);

    let mut ids = BTreeSet::new();
    let mut supported_expected = 0_usize;
    let mut supported_accepted = 0_usize;
    let mut false_rejections = 0_usize;
    let mut rejected_expected = 0_usize;
    let mut rejected_actual = 0_usize;
    let mut false_acceptances = 0_usize;
    let mut exact_matches = 0_usize;
    let mut by_category = BTreeMap::<String, CategoryMetrics>::new();
    let mut mismatches = Vec::new();

    for case in &corpus.cases {
        assert!(ids.insert(&case.id), "duplicate case ID {}", case.id);
        let category = by_category.entry(case.category.clone()).or_default();
        category.cases += 1;
        let result = analyze_sql(&case.sql);
        let exact = match (case.expected.decision.as_str(), result) {
            ("accept", Ok(analyzed)) => {
                supported_expected += 1;
                supported_accepted += 1;
                let matches = case.expected.statement_kind.as_deref()
                    == Some(analyzed.kind.as_str())
                    && case.expected.requested_table.as_deref()
                        == Some(analyzed.requested_table.as_str())
                    && case.expected.inserted_rows == Some(analyzed.inserted_rows);
                if !matches {
                    mismatches.push(format!(
                        "{} expected {:?}/{:?}/{:?}, got {}/{}/{}",
                        case.id,
                        case.expected.statement_kind,
                        case.expected.requested_table,
                        case.expected.inserted_rows,
                        analyzed.kind.as_str(),
                        analyzed.requested_table,
                        analyzed.inserted_rows
                    ));
                }
                matches
            }
            ("accept", Err(error)) => {
                supported_expected += 1;
                false_rejections += 1;
                mismatches.push(format!(
                    "{} expected acceptance but got {}",
                    case.id, error.code
                ));
                false
            }
            ("reject", Err(error)) => {
                rejected_expected += 1;
                rejected_actual += 1;
                let matches = case.expected.error_code.as_deref() == Some(error.code.as_str());
                if !matches {
                    mismatches.push(format!(
                        "{} expected rejection code {:?}, got {}",
                        case.id, case.expected.error_code, error.code
                    ));
                }
                matches
            }
            ("reject", Ok(analyzed)) => {
                rejected_expected += 1;
                false_acceptances += 1;
                mismatches.push(format!(
                    "{} expected rejection but was accepted as {}",
                    case.id,
                    analyzed.kind.as_str()
                ));
                false
            }
            (decision, _) => {
                mismatches.push(format!(
                    "{} has unsupported expected decision {decision}",
                    case.id
                ));
                false
            }
        };
        if exact {
            exact_matches += 1;
            category.exact_matches += 1;
        } else if !mismatches
            .iter()
            .any(|message| message.starts_with(&case.id))
        {
            mismatches.push(format!("{} returned a different classification", case.id));
        }
    }

    let supported_rate = ratio(supported_accepted, supported_expected);
    let rejection_rate = ratio(rejected_actual, rejected_expected);
    assert!(
        supported_rate >= corpus.requirements.required_supported_acceptance,
        "supported acceptance rate {supported_rate}; first mismatches: {:?}",
        mismatches.iter().take(20).collect::<Vec<_>>()
    );
    assert!(
        rejection_rate >= corpus.requirements.required_unsupported_rejection,
        "unsupported rejection rate {rejection_rate}; first mismatches: {:?}",
        mismatches.iter().take(20).collect::<Vec<_>>()
    );

    let category_json = by_category
        .into_iter()
        .map(|(name, metrics)| {
            (
                name,
                json!({
                    "cases": metrics.cases,
                    "exact_matches": metrics.exact_matches,
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let actual_metrics = json!({
        "schema_version": "dmlpact.sql-policy-metrics/v0.1",
        "corpus_sha256": sha256_bytes(&corpus_bytes),
        "total_cases": corpus.cases.len(),
        "exact_matches": exact_matches,
        "classification_accuracy": ratio(exact_matches, corpus.cases.len()),
        "supported": {
            "expected": supported_expected,
            "accepted": supported_accepted,
            "false_rejections": false_rejections,
            "acceptance_rate": supported_rate,
        },
        "unsupported_or_ambiguous": {
            "expected": rejected_expected,
            "rejected": rejected_actual,
            "false_acceptances": false_acceptances,
            "rejection_rate": rejection_rate,
        },
        "by_category": category_json,
    });
    assert_eq!(
        actual_metrics,
        read_json(&root.join("metrics.json")),
        "pinned SQL policy metrics changed; first mismatches: {:?}",
        mismatches.iter().take(20).collect::<Vec<_>>()
    );
}
