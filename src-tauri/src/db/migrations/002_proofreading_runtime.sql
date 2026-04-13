ALTER TABLE proofreading_jobs
ADD COLUMN options_json TEXT;

ALTER TABLE proofreading_jobs
ADD COLUMN auto_resume INTEGER NOT NULL DEFAULT 1;
