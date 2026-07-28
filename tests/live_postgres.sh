#!/usr/bin/env bash
set -euo pipefail

binary="${1:-target/debug/dmlpact}"
test_root="$(mktemp -d)"
trap 'rm -rf "${test_root}"' EXIT

export DMLPACT_DATABASE_URL="${DMLPACT_TEST_DATABASE_URL:-postgresql://postgres:postgres@127.0.0.1:5432/postgres}"

psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 <<'SQL' >/dev/null
DROP TABLE IF EXISTS dmlpact_accounts CASCADE;
CREATE TABLE dmlpact_accounts (
  id bigint PRIMARY KEY,
  active boolean NOT NULL,
  note text
);
INSERT INTO dmlpact_accounts VALUES
  (1, true, 'a'),
  (2, true, 'b'),
  (3, true, 'c');
SQL

printf '%s\n' \
  'UPDATE dmlpact_accounts SET active = false WHERE id IN (1, 2)' \
  > "${test_root}/update.sql"
"${binary}" plan \
  --sql "${test_root}/update.sql" \
  --out "${test_root}/update.plan.json" \
  --max-rows 2 \
  --allow-insecure-localhost \
  > "${test_root}/plan-output.json"
"${binary}" apply \
  --sql "${test_root}/update.sql" \
  --plan "${test_root}/update.plan.json" \
  --receipt "${test_root}/update.receipt.ndjson" \
  --allow-insecure-localhost \
  > "${test_root}/apply-output.json"
"${binary}" receipt verify \
  --receipt "${test_root}/update.receipt.ndjson" \
  > "${test_root}/verification.json"

jq -e '.preconditions.target_count == 2' "${test_root}/plan-output.json" >/dev/null
jq -e '.state == "committed" and .affected_rows == 2' \
  "${test_root}/apply-output.json" >/dev/null
jq -e '.integrity_valid and .complete and .final_state == "committed"' \
  "${test_root}/verification.json" >/dev/null
test "$(psql "${DMLPACT_DATABASE_URL}" -Atc \
  'SELECT count(*) FROM dmlpact_accounts WHERE active = false')" = "2"

printf '%s\n' \
  'DELETE FROM dmlpact_accounts WHERE active = true' \
  > "${test_root}/drift.sql"
"${binary}" plan \
  --sql "${test_root}/drift.sql" \
  --out "${test_root}/drift.plan.json" \
  --max-rows 2 \
  --allow-insecure-localhost \
  >/dev/null
psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 \
  -c "INSERT INTO dmlpact_accounts VALUES (4, true, 'd')" >/dev/null
"${binary}" apply \
  --sql "${test_root}/drift.sql" \
  --plan "${test_root}/drift.plan.json" \
  --receipt "${test_root}/drift.receipt.ndjson" \
  --allow-insecure-localhost \
  > "${test_root}/drift-output.json"
"${binary}" receipt verify \
  --receipt "${test_root}/drift.receipt.ndjson" \
  > "${test_root}/drift-verification.json"
jq -e '.state == "refused" and .reason_code == "target_count_drift"' \
  "${test_root}/drift-output.json" >/dev/null
jq -e '.integrity_valid and .complete and .final_state == "refused"' \
  "${test_root}/drift-verification.json" >/dev/null
test "$(psql "${DMLPACT_DATABASE_URL}" -Atc \
  'SELECT count(*) FROM dmlpact_accounts WHERE active = true')" = "2"

printf '%s\n' \
  'UPDATE dmlpact_accounts SET note = '"'"'selected'"'"' WHERE active = true' \
  > "${test_root}/target-set.sql"
"${binary}" plan \
  --sql "${test_root}/target-set.sql" \
  --out "${test_root}/target-set.plan.json" \
  --max-rows 2 \
  --allow-insecure-localhost \
  >/dev/null
psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 \
  -c 'UPDATE dmlpact_accounts SET active = false WHERE id = 3' \
  -c 'UPDATE dmlpact_accounts SET active = true WHERE id = 1' \
  >/dev/null
"${binary}" apply \
  --sql "${test_root}/target-set.sql" \
  --plan "${test_root}/target-set.plan.json" \
  --receipt "${test_root}/target-set.receipt.ndjson" \
  --allow-insecure-localhost \
  > "${test_root}/target-set-output.json"
jq -e '.state == "refused" and .reason_code == "target_set_drift"' \
  "${test_root}/target-set-output.json" >/dev/null
test "$(psql "${DMLPACT_DATABASE_URL}" -Atc \
  "SELECT count(*) FROM dmlpact_accounts WHERE note = 'selected'")" = "0"

psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 <<'SQL' >/dev/null
CREATE FUNCTION dmlpact_touch() RETURNS trigger
LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END $$;
CREATE TRIGGER dmlpact_trigger
BEFORE UPDATE ON dmlpact_accounts
FOR EACH ROW EXECUTE FUNCTION dmlpact_touch();
SQL
printf '%s\n' \
  'UPDATE dmlpact_accounts SET note = '"'"'reviewed'"'"' WHERE id = 3' \
  > "${test_root}/trigger.sql"
set +e
"${binary}" plan \
  --sql "${test_root}/trigger.sql" \
  --out "${test_root}/trigger.plan.json" \
  --max-rows 1 \
  --allow-insecure-localhost \
  > "${test_root}/trigger-stdout.json" \
  2> "${test_root}/trigger-error.json"
trigger_exit=$?
set -e
test "${trigger_exit}" = "3"
jq -e '.code == "user_triggers_denied"' "${test_root}/trigger-error.json" >/dev/null
test ! -e "${test_root}/trigger.plan.json"

psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 <<'SQL' >/dev/null
CREATE TABLE dmlpact_defaulted (
  id bigint DEFAULT 1,
  note text
);
SQL
printf '%s\n' \
  "INSERT INTO dmlpact_defaulted (note) VALUES ('hidden default')" \
  > "${test_root}/default.sql"
set +e
"${binary}" plan \
  --sql "${test_root}/default.sql" \
  --out "${test_root}/default.plan.json" \
  --max-rows 1 \
  --allow-insecure-localhost \
  > "${test_root}/default-stdout.json" \
  2> "${test_root}/default-error.json"
default_exit=$?
set -e
test "${default_exit}" = "3"
jq -e '.code == "insert_implicit_behavior_denied"' \
  "${test_root}/default-error.json" >/dev/null
test ! -e "${test_root}/default.plan.json"

psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 <<'SQL' >/dev/null
CREATE TABLE dmlpact_parent (id bigint PRIMARY KEY);
CREATE TABLE dmlpact_child (
  id bigint PRIMARY KEY,
  parent_id bigint REFERENCES dmlpact_parent(id) ON DELETE CASCADE
);
INSERT INTO dmlpact_parent VALUES (1);
INSERT INTO dmlpact_child VALUES (1, 1);
SQL
printf '%s\n' \
  'DELETE FROM dmlpact_parent WHERE id = 1' \
  > "${test_root}/cascade.sql"
set +e
"${binary}" plan \
  --sql "${test_root}/cascade.sql" \
  --out "${test_root}/cascade.plan.json" \
  --max-rows 1 \
  --allow-insecure-localhost \
  > "${test_root}/cascade-stdout.json" \
  2> "${test_root}/cascade-error.json"
cascade_exit=$?
set -e
test "${cascade_exit}" = "3"
jq -e '.code == "referential_actions_denied"' \
  "${test_root}/cascade-error.json" >/dev/null
test ! -e "${test_root}/cascade.plan.json"
