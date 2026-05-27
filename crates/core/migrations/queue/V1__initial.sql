-- Durable queue store for background processing.
CREATE TABLE _queue_job (
  id            BLOB PRIMARY KEY DEFAULT (uuid_v7()) NOT NULL,
  queue         TEXT NOT NULL,
  job_type      TEXT NOT NULL,
  payload       TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload)),

  status        TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending', 'running', 'succeeded', 'failed', 'dead')),
  priority      INTEGER NOT NULL DEFAULT 100,

  attempts      INTEGER NOT NULL DEFAULT 0,
  max_attempts  INTEGER NOT NULL DEFAULT 5 CHECK (max_attempts > 0),

  run_at        REAL NOT NULL DEFAULT (UNIXEPOCH('subsec')),
  lease_until   REAL,
  worker_id     TEXT,

  org_id        BLOB,
  idempotency_key TEXT,

  last_error    TEXT,
  created       REAL NOT NULL DEFAULT (UNIXEPOCH('subsec')),
  updated       REAL NOT NULL DEFAULT (UNIXEPOCH('subsec'))
) STRICT;

CREATE UNIQUE INDEX __queue_job__idempotency_index
  ON _queue_job (queue, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE INDEX __queue_job__status_run_at_index
  ON _queue_job (status, run_at, priority);

CREATE INDEX __queue_job__queue_status_index
  ON _queue_job (queue, status, run_at);

CREATE INDEX __queue_job__org_status_index
  ON _queue_job (org_id, status, run_at);

CREATE TABLE _queue_job_attempt (
  id            BLOB PRIMARY KEY DEFAULT (uuid_v7()) NOT NULL,
  job_id        BLOB NOT NULL REFERENCES _queue_job(id) ON DELETE CASCADE,
  attempt_no    INTEGER NOT NULL,
  status        TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
  error         TEXT,
  started       REAL NOT NULL DEFAULT (UNIXEPOCH('subsec')),
  finished      REAL,
  worker_id     TEXT,
  UNIQUE (job_id, attempt_no)
) STRICT;

CREATE INDEX __queue_attempt__job_id_index ON _queue_job_attempt (job_id);

CREATE TABLE _queue_dead_letter (
  id            BLOB PRIMARY KEY DEFAULT (uuid_v7()) NOT NULL,
  job_id        BLOB NOT NULL,
  queue         TEXT NOT NULL,
  job_type      TEXT NOT NULL,
  payload       TEXT NOT NULL,
  attempts      INTEGER NOT NULL,
  failed_at     REAL NOT NULL DEFAULT (UNIXEPOCH('subsec')),
  last_error    TEXT
) STRICT;

CREATE INDEX __queue_dead__failed_at_index ON _queue_dead_letter (failed_at);
