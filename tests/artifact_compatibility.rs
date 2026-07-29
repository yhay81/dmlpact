#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use dmlpact::integrity::{sha256_bytes, sha256_json};
use dmlpact::model::{ReceiptEvent, TOOL_VERSION};
use dmlpact::receipt::{read_plan, verify_receipt};
use serde_json::Value;

const RELEASE_CORPORA: &[(&str, &str)] = &[("v0.1", "0.1.0"), ("v0.2", "0.2.0"), ("v0.3", "0.3.0")];

fn corpus_root(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/contracts")
        .join(version)
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read JSON fixture")).expect("parse JSON fixture")
}

fn mutate(document: &mut Value, operation: &str, pointer: &str, value: Value) {
    match operation {
        "replace" => *document.pointer_mut(pointer).expect("replace target") = value,
        "add" => {
            let (parent_pointer, key) = pointer.rsplit_once('/').expect("pointer parent");
            let parent = if parent_pointer.is_empty() {
                document
            } else {
                document
                    .pointer_mut(parent_pointer)
                    .expect("mutation parent")
            };
            assert!(parent
                .as_object_mut()
                .expect("object parent")
                .insert(key.to_owned(), value)
                .is_none());
        }
        other => panic!("unsupported mutation {other}"),
    }
}

#[test]
fn current_readers_accept_every_released_artifact_corpus() {
    let mut covered_tool_versions = BTreeSet::new();

    for (corpus_version, expected_tool_version) in RELEASE_CORPORA {
        let root = corpus_root(corpus_version);
        let manifest = read_json(&root.join("manifest.json"));
        assert_eq!(manifest["schema_version"], "dmlpact.artifact-corpus/v1");
        let mut paths = BTreeSet::new();
        for entry in manifest["accepted"].as_array().expect("accepted entries") {
            let relative = entry["path"].as_str().expect("accepted path");
            assert!(paths.insert(relative.to_owned()));
            let bytes = fs::read(root.join(relative)).expect("read accepted artifact");
            assert_eq!(
                sha256_bytes(&bytes),
                entry["sha256"],
                "digest for {corpus_version}/{relative}"
            );
        }
        assert_eq!(
            paths,
            BTreeSet::from([
                "delete.plan.json".to_owned(),
                "delete.receipt.ndjson".to_owned()
            ])
        );

        let plan_path = root.join("delete.plan.json");
        let plan_bytes = fs::read(&plan_path).expect("read plan");
        let plan = read_plan(&plan_path).expect("verify golden plan");
        assert_eq!(plan.tool_version, *expected_tool_version);
        assert!(covered_tool_versions.insert(plan.tool_version.clone()));
        assert_eq!(
            format!(
                "{}\n",
                serde_json::to_string_pretty(&plan).expect("serialize plan")
            )
            .as_bytes(),
            plan_bytes
        );

        let receipt_path = root.join("delete.receipt.ndjson");
        let receipt_bytes = fs::read(&receipt_path).expect("read receipt");
        let events = receipt_bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice::<ReceiptEvent>(line).expect("parse receipt event"))
            .collect::<Vec<_>>();
        assert!(events
            .iter()
            .all(|event| event.tool_version == *expected_tool_version));
        assert_eq!(
            events[0].plan_sha256,
            plan.plan_sha256.clone().expect("plan hash")
        );
        let serialized = events
            .iter()
            .map(|event| serde_json::to_string(event).expect("serialize receipt event"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(serialized.as_bytes(), receipt_bytes);
        let verification = verify_receipt(&receipt_path).expect("verify golden receipt");
        assert!(verification.integrity_valid);
        assert!(verification.complete);
        assert_eq!(verification.event_count, 2);
    }

    assert!(
        covered_tool_versions.contains(TOOL_VERSION),
        "the current release must have a digest-pinned artifact corpus"
    );
}

#[test]
fn declared_v01_mutations_fail_closed() {
    let root = corpus_root("v0.1");
    let manifest = read_json(&root.join("manifest.json"));
    let mut ids = BTreeSet::new();

    for case in manifest["rejections"].as_array().expect("rejection cases") {
        let id = case["id"].as_str().expect("case ID");
        assert!(ids.insert(id.to_owned()));
        let directory = tempfile::tempdir().expect("mutation directory");
        let target = case["target"].as_str().expect("mutation target");
        let error = if target == "plan" {
            let mut document = read_json(&root.join("delete.plan.json"));
            mutate(
                &mut document,
                case["operation"].as_str().expect("operation"),
                case["pointer"].as_str().expect("pointer"),
                case["value"].clone(),
            );
            let path = directory.path().join("plan.json");
            fs::write(
                &path,
                format!(
                    "{}\n",
                    serde_json::to_string_pretty(&document).expect("serialize plan mutation")
                ),
            )
            .expect("write plan mutation");
            read_plan(&path).expect_err("plan mutation must fail")
        } else {
            let mut events = fs::read_to_string(root.join("delete.receipt.ndjson"))
                .expect("read receipt")
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).expect("parse receipt line"))
                .collect::<Vec<_>>();
            let index = case["event_index"].as_u64().expect("event index") as usize;
            mutate(
                &mut events[index],
                case["operation"].as_str().expect("operation"),
                case["pointer"].as_str().expect("pointer"),
                case["value"].clone(),
            );
            if case["rebind_event"].as_bool().unwrap_or(false) {
                let mut event: ReceiptEvent = serde_json::from_value(events[index].clone())
                    .expect("deserialize rebound event");
                event.event_sha256 = None;
                events[index]["event_sha256"] =
                    Value::String(sha256_json(&event).expect("hash rebound event"));
            }
            let path = directory.path().join("receipt.ndjson");
            fs::write(
                &path,
                events
                    .iter()
                    .map(|event| serde_json::to_string(event).expect("serialize receipt mutation"))
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n",
            )
            .expect("write receipt mutation");
            verify_receipt(&path).expect_err("receipt mutation must fail")
        };
        assert_eq!(error.code, case["expected_code"], "rejection {id}");
    }
    assert_eq!(ids.len(), 12);
}
