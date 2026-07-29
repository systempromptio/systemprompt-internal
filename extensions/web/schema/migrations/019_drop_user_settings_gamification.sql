-- 2026-07-24: drop the gamification columns from user_settings.
--
-- The settings page no longer offers the Daily Summary, Achievements, and
-- Leaderboard Visibility toggles, and the daily_summary job that was the only
-- consumer of notify_daily_summary (and of the two email bookkeeping dates) had
-- no callers and has been deleted. The declarative CREATE in
-- 10_admin_dashboard.sql was narrowed in the same change; this migration brings
-- existing databases in line. Forward-only and idempotent.

ALTER TABLE user_settings DROP COLUMN IF EXISTS notify_daily_summary;
ALTER TABLE user_settings DROP COLUMN IF EXISTS notify_achievements;
ALTER TABLE user_settings DROP COLUMN IF EXISTS leaderboard_opt_in;
ALTER TABLE user_settings DROP COLUMN IF EXISTS achievement_email_sent_date;
ALTER TABLE user_settings DROP COLUMN IF EXISTS daily_report_email_sent_date;
