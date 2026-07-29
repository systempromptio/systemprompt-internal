-- What a plan is billed at, alongside what it is allowed to spend.
--
-- plans.monthly_cost_cap_microdollars is a ceiling on inference cost. It says
-- nothing about revenue, so an operator looking at the enterprise list could
-- see what every customer consumed and not what any of them was worth. The
-- price closes that gap: margin = price - cost, per organization, per month.
--
-- Authored in services/access-control/plans.yaml as `monthly_price_usd` and
-- stored in microdollars so it shares units with ai_requests.cost_microdollars
-- and needs no conversion at query time.

ALTER TABLE plans
    ADD COLUMN IF NOT EXISTS monthly_price_microdollars BIGINT NOT NULL DEFAULT 0;
