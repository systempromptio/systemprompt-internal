-- 2026-07-24: give every user an explicit department row.
--
-- "Unassigned" was a label the access-matrix page invented for users whose
-- user_profile_ext.department was NULL or ''. It was never a row in
-- `departments`, so the matrix tree and the departments list disagreed about
-- which departments exist, and the matrix's assign-department <select> — built
-- from the membership-derived list with "Unassigned" filtered out — rendered
-- empty, leaving the department write path unreachable. `Default` (seeded by
-- 009) is the row the delete path already reassigns members to, so it becomes
-- the single name for "no explicit department". Forward-only and idempotent.

UPDATE user_profile_ext
   SET department = 'Default'
 WHERE department IS NULL OR department = '';

-- Users with no profile row at all were counted as "Unassigned" by the tree and
-- were invisible to the departments list, which joins through user_profile_ext.
INSERT INTO user_profile_ext (user_id, department)
SELECT u.id, 'Default'
  FROM users u
 WHERE NOT ('anonymous' = ANY(u.roles))
   AND u.email NOT LIKE '%@anonymous.local'
   AND NOT EXISTS (SELECT 1 FROM user_profile_ext upe WHERE upe.user_id = u.id)
    ON CONFLICT (user_id) DO NOTHING;
