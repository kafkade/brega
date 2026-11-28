-- Optional weekly time budget per area in whole minutes (issue #229, R-11).
--
-- NULL means "no budget". Synced like every other areas column.
ALTER TABLE areas ADD COLUMN weekly_budget_minutes INTEGER
    CHECK (weekly_budget_minutes IS NULL OR weekly_budget_minutes > 0);
