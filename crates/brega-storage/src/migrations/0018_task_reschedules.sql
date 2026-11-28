-- Append-only log of a task's planned day or due date moving later
-- (issue #230). Input to the Weekly Review "slipped" count (R-12).
--
-- Sync events are synthesized from state diffs at sync time, so several
-- reschedules between syncs would collapse into one diff; recording each
-- move as its own row keeps them countable. Rows are never updated or
-- deleted by the app and sync as independent entities, so concurrent
-- reschedules on two devices are both kept. No foreign key: a row may
-- arrive by sync before its task.
CREATE TABLE task_reschedules (
    id          BLOB PRIMARY KEY,
    task_id     BLOB NOT NULL,
    field       TEXT NOT NULL CHECK (field IN ('scheduled_for', 'deadline')),
    from_slot   TEXT,
    to_slot     TEXT,
    created_at  TEXT NOT NULL
);

CREATE INDEX task_reschedules_task_idx ON task_reschedules(task_id);
