\set ON_ERROR_STOP on

DROP TABLE IF EXISTS dmlpact_benchmark;
CREATE TABLE dmlpact_benchmark (
  id bigint PRIMARY KEY,
  selected boolean NOT NULL,
  benchmark_value integer NOT NULL,
  note text NOT NULL
);
INSERT INTO dmlpact_benchmark (id, selected, benchmark_value, note)
SELECT
  value,
  value <= 10000,
  0,
  'synthetic-' || lpad(value::text, 6, '0')
FROM generate_series(1, 100000) AS value;
ANALYZE dmlpact_benchmark;
