-- Rename the seeded house organization and plan away from the old brand.
--
-- Migration 022 seeded the house org as "Astound Digital" and its plan
-- description as "Astound-internal". Both are user-visible — the org name heads
-- the enterprise console and appears on every member's profile — and the fork
-- no longer carries that brand.
--
-- Done forward rather than by editing 022, because 022 has already applied on
-- existing installs and rewriting it would only change the recorded checksum,
-- not the rows. The name is matched explicitly so an operator who has already
-- renamed the org keeps their choice.

UPDATE organizations
SET name = 'Systemprompt Internal'
WHERE id = 'house' AND name = 'Astound Digital';

UPDATE plans
SET description = 'Internal. Unlimited seats, no spend cap.'
WHERE id = 'house' AND description = 'Astound-internal. Unlimited seats, no spend cap.';
