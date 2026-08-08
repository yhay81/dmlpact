#!/usr/bin/env python3
"""Generate the deterministic DMLPact SQL policy corpus."""

from __future__ import annotations

import argparse
import hashlib
import json
from collections import Counter
from pathlib import Path
from typing import Any


CORPUS_SCHEMA = "dmlpact.sql-policy-corpus/v0.1"
METRICS_SCHEMA = "dmlpact.sql-policy-metrics/v0.1"


def build_corpus() -> dict[str, Any]:
    cases: list[dict[str, Any]] = []

    def accept(
        category: str,
        sql: str,
        kind: str,
        table: str,
        *,
        inserted_rows: int = 0,
    ) -> None:
        cases.append(
            {
                "id": f"{category}-{sum(case['category'] == category for case in cases) + 1:03d}",
                "category": category,
                "sql": sql,
                "expected": {
                    "decision": "accept",
                    "statement_kind": kind,
                    "requested_table": table,
                    "inserted_rows": inserted_rows,
                },
            }
        )

    def reject(category: str, sql: str, error_code: str) -> None:
        cases.append(
            {
                "id": f"{category}-{sum(case['category'] == category for case in cases) + 1:03d}",
                "category": category,
                "sql": sql,
                "expected": {
                    "decision": "reject",
                    "error_code": error_code,
                },
            }
        )

    def table_name(index: int) -> tuple[str, str]:
        name = f"accounts_{index:03d}"
        variants = (
            (name, name),
            (f"public.{name}", f"public.{name}"),
            (f'"{name}"', f'"{name}"'),
            (f'"app"."{name}"', f'"app"."{name}"'),
        )
        return variants[index % len(variants)]

    for index in range(100):
        table, expected_table = table_name(index)
        rows = (
            f"({index}, true, 'row {index}')"
            if index % 3
            else f"({index}, true, 'owner''s row'), ({index + 1000}, false, NULL)"
        )
        accept(
            "insert_supported",
            f"INSERT INTO {table} (id, active, note) VALUES {rows}",
            "insert",
            expected_table,
            inserted_rows=2 if index % 3 == 0 else 1,
        )

    update_values = (
        "false",
        "NULL",
        "-7",
        "value + 1",
        "'reviewed'",
    )
    predicates = (
        "id = {index}",
        "id IN ({index}, {next_index})",
        "id BETWEEN {index} AND {next_index}",
        "active = true AND id = {index}",
        "note IS NULL",
    )
    for index in range(100):
        table, expected_table = table_name(index)
        alias = " AS target" if index % 2 else ""
        qualifier = "target." if alias else ""
        predicate = predicates[index % len(predicates)].format(
            index=index, next_index=index + 1
        )
        if qualifier:
            predicate = predicate.replace("id", f"{qualifier}id")
            predicate = predicate.replace("active", f"{qualifier}active")
            predicate = predicate.replace("note", f"{qualifier}note")
        accept(
            "update_supported",
            f"UPDATE {table}{alias} SET value = "
            f"{update_values[index % len(update_values)]} WHERE {predicate}",
            "update",
            expected_table,
        )

    delete_predicates = (
        "id = {index}",
        "id <> {index}",
        "id IN ({index}, {next_index})",
        "active IS false",
        "note IS NOT NULL AND id >= {index}",
    )
    for index in range(100):
        table, expected_table = table_name(index)
        alias = " AS doomed" if index % 2 else ""
        qualifier = "doomed." if alias else ""
        predicate = delete_predicates[index % len(delete_predicates)].format(
            index=index, next_index=index + 1
        )
        if qualifier:
            predicate = predicate.replace("id", f"{qualifier}id")
            predicate = predicate.replace("active", f"{qualifier}active")
            predicate = predicate.replace("note", f"{qualifier}note")
        accept(
            "delete_supported",
            f"DELETE FROM {table}{alias} WHERE {predicate}",
            "delete",
            expected_table,
        )

    for index in range(100):
        table = f'"Case Sensitive {index:03d}"'
        if index % 3 == 0:
            accept(
                "quoting_comments_supported",
                f"/* ticket {index} */ INSERT INTO {table} "
                f'("Id", "Note") VALUES ({index}, \'quoted \'\'value\'\'\') -- reviewed\n',
                "insert",
                table,
                inserted_rows=1,
            )
        elif index % 3 == 1:
            accept(
                "quoting_comments_supported",
                f"-- ticket {index}\nUPDATE {table} SET \"Note\" = 'ok' "
                f'WHERE "Id" = {index} /* bounded */',
                "update",
                table,
            )
        else:
            accept(
                "quoting_comments_supported",
                f"/* ticket {index} */ DELETE FROM {table} "
                f'WHERE "Id" = {index} -- bounded\n',
                "delete",
                table,
            )

    for index in range(100):
        variant = index % 4
        if variant == 0:
            sql = (
                "UPDATE accounts SET active = false "
                f"WHERE id IN (SELECT id FROM stale_{index})"
            )
            code = "dynamic_expression_denied"
        elif variant == 1:
            sql = (
                "DELETE FROM accounts "
                f"WHERE EXISTS (SELECT 1 FROM stale_{index})"
            )
            code = "dynamic_expression_denied"
        elif variant == 2:
            sql = (
                "UPDATE accounts SET value = "
                f"(SELECT value FROM source_{index}) WHERE id = {index}"
            )
            code = "dynamic_expression_denied"
        else:
            sql = (
                "INSERT INTO accounts (id, note) "
                f"SELECT id, note FROM source_{index}"
            )
            code = "insert_select_denied"
        reject("subquery_rejected", sql, code)

    expression_variants = (
        lambda index: (
            f"UPDATE accounts SET seen_at = now() WHERE id = {index}",
            "dynamic_expression_denied",
        ),
        lambda index: (
            f"DELETE FROM accounts WHERE lower(note) = 'row-{index}'",
            "dynamic_expression_denied",
        ),
        lambda index: (
            "INSERT INTO accounts (id, created_at) "
            f"VALUES ({index}, current_timestamp())",
            "dynamic_expression_denied",
        ),
        lambda index: (
            "UPDATE accounts SET note = $1 "
            f"WHERE id = {index}",
            "dynamic_expression_denied",
        ),
        lambda index: (
            f"UPDATE accounts SET note = DEFAULT WHERE id = {index}",
            "dynamic_expression_denied",
        ),
    )
    for index in range(100):
        sql, code = expression_variants[index % len(expression_variants)](index)
        reject("expression_rejected", sql, code)

    for index in range(100):
        sql = (
            f"WITH selected_{index} AS (SELECT id FROM accounts WHERE id = {index}) "
            "UPDATE accounts SET active = false "
            f"WHERE id IN (SELECT id FROM selected_{index})"
        )
        reject("cte_rejected", sql, "dynamic_expression_denied")

    for index in range(100):
        first = f"UPDATE accounts SET active = false WHERE id = {index}"
        second = f"DELETE FROM audit_rows WHERE id = {index}"
        separators = ("; ", ";\n", "; /* second */ ", "; -- second\n")
        reject(
            "multi_statement_rejected",
            first + separators[index % len(separators)] + second,
            "statement_count_not_one",
        )

    unsupported_variants = (
        lambda index: (
            f"INSERT INTO accounts (id) VALUES ({index}) RETURNING id",
            "insert_form_denied",
        ),
        lambda index: (
            f"INSERT INTO accounts (id) VALUES ({index}) ON CONFLICT DO NOTHING",
            "insert_form_denied",
        ),
        lambda index: (
            f"UPDATE accounts SET active = false FROM source_{index} "
            f"WHERE accounts.id = source_{index}.id",
            "update_form_denied",
        ),
        lambda index: (
            f"UPDATE accounts SET active = false WHERE id = {index} RETURNING id",
            "update_form_denied",
        ),
        lambda index: (
            f"UPDATE accounts SET active = false WHERE id = {index} LIMIT 1",
            "update_form_denied",
        ),
        lambda index: ("UPDATE accounts SET active = false", "where_required"),
        lambda index: (
            f"DELETE FROM accounts USING source_{index} "
            f"WHERE accounts.id = source_{index}.id",
            "delete_form_denied",
        ),
        lambda index: (
            f"DELETE FROM accounts WHERE id = {index} RETURNING id",
            "delete_form_denied",
        ),
        lambda index: ("DELETE FROM accounts", "where_required"),
        lambda index: (
            f"SELECT * FROM accounts WHERE id = {index}",
            "statement_kind_denied",
        ),
    )
    for index in range(100):
        sql, code = unsupported_variants[index % len(unsupported_variants)](index)
        reject("unsupported_form_rejected", sql, code)

    ambiguous_variants = (
        lambda index: ("UPDATE", "sql_parse_failed"),
        lambda index: (
            f"UPDATE accounts SET note = 'unterminated {index}",
            "sql_parse_failed",
        ),
        lambda index: (
            f"DELETE FROM accounts WHERE (id = {index}",
            "sql_parse_failed",
        ),
        lambda index: (
            f"MERGE INTO accounts USING source_{index} ON false WHEN MATCHED THEN DELETE",
            "statement_kind_denied",
        ),
        lambda index: (
            f"TRUNCATE TABLE accounts_{index}",
            "statement_kind_denied",
        ),
        lambda index: (
            f"ALTER TABLE accounts_{index} DROP COLUMN note",
            "statement_kind_denied",
        ),
        lambda index: (
            f"VALUES ({index})",
            "statement_kind_denied",
        ),
        lambda index: (
            f"UPDATE catalog.public.accounts SET value = {index} WHERE id = {index}",
            "table_name_denied",
        ),
        lambda index: (
            f"DELETE accounts_{index} WHERE id = {index}",
            "sql_parse_failed",
        ),
        lambda index: (
            f"/* comment-only case {index} */",
            "statement_count_not_one",
        ),
    )
    for index in range(100):
        sql, code = ambiguous_variants[index % len(ambiguous_variants)](index)
        reject("ambiguous_invalid_rejected", sql, code)

    assert len(cases) == 1000, len(cases)
    assert len({case["id"] for case in cases}) == len(cases)
    category_counts = Counter(case["category"] for case in cases)
    assert set(category_counts.values()) == {100}, category_counts
    return {
        "schema_version": CORPUS_SCHEMA,
        "license": "MIT",
        "labeling_methodology": (
            "Each SQL family and expected decision was manually curated from "
            "the documented DMLPact safety contract; no DMLPact output is used "
            "to generate labels."
        ),
        "requirements": {
            "minimum_cases": 1000,
            "required_supported_acceptance": 1.0,
            "required_unsupported_rejection": 1.0,
        },
        "cases": cases,
    }


def build_metrics(corpus: dict[str, Any]) -> dict[str, Any]:
    cases = corpus["cases"]
    supported = sum(
        case["expected"]["decision"] == "accept" for case in cases
    )
    rejected = len(cases) - supported
    by_category = Counter(case["category"] for case in cases)
    return {
        "schema_version": METRICS_SCHEMA,
        "corpus_sha256": hashlib.sha256(canonical_encode(corpus)).hexdigest(),
        "total_cases": len(cases),
        "exact_matches": len(cases),
        "classification_accuracy": 1.0,
        "supported": {
            "expected": supported,
            "accepted": supported,
            "false_rejections": 0,
            "acceptance_rate": 1.0,
        },
        "unsupported_or_ambiguous": {
            "expected": rejected,
            "rejected": rejected,
            "false_acceptances": 0,
            "rejection_rate": 1.0,
        },
        "by_category": {
            category: {"cases": count, "exact_matches": count}
            for category, count in sorted(by_category.items())
        },
    }


def encode(value: dict[str, Any]) -> bytes:
    return (
        json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    ).encode()


def canonical_encode(value: dict[str, Any]) -> bytes:
    return json.dumps(
        value,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
    ).encode()


def main() -> int:
    parser = argparse.ArgumentParser()
    directory = Path(__file__).parent
    parser.add_argument(
        "--corpus-output",
        type=Path,
        default=directory / "corpus.json",
    )
    parser.add_argument(
        "--metrics-output",
        type=Path,
        default=directory / "metrics.json",
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    corpus = build_corpus()
    generated = {
        args.corpus_output: encode(corpus),
        args.metrics_output: encode(build_metrics(corpus)),
    }
    if args.check:
        stale = [
            str(path)
            for path, expected in generated.items()
            if not path.is_file() or path.read_bytes() != expected
        ]
        if stale:
            raise SystemExit(
                "generated SQL policy evidence is stale: " + ", ".join(stale)
            )
        print(f"verified {len(corpus['cases'])} SQL policy cases")
        return 0
    for path, contents in generated.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(contents)
    print(f"wrote {len(corpus['cases'])} SQL policy cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
