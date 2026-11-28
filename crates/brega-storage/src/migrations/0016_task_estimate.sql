-- Optional task effort estimate in whole minutes (issue #228, R-10).
--
-- NULL means "not estimated". Synced like every other tasks column.
ALTER TABLE tasks ADD COLUMN estimate_minutes INTEGER
    CHECK (estimate_minutes IS NULL OR estimate_minutes > 0);
