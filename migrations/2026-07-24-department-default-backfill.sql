-- Forward-only, idempotent. Safe to re-run.
-- Department normalisation (2026-07-24): "Unassigned" was a label invented by
-- the access-matrix page for users whose user_profile_ext.department was NULL
-- or ''. It was never a row in `departments`, so the matrix tree and the
-- departments list disagreed about which departments exist, and the matrix's
-- assign-department <select> rendered empty — making the department write path
-- unreachable. `Default` is the real row the delete path already reassigns to,
-- so it becomes the single name for "no explicit department".

INSERT INTO departments (name, description)
SELECT 'Default', 'Default department; contains every user without an explicit assignment.'
WHERE NOT EXISTS (SELECT 1 FROM departments WHERE name = 'Default');

-- Users that have a profile row but no department.
UPDATE user_profile_ext
   SET department = 'Default'
 WHERE department IS NULL OR department = '';

-- Users that have no profile row at all: they were counted as "Unassigned" by
-- the tree and were invisible to the departments list, which joins through
-- user_profile_ext.
INSERT INTO user_profile_ext (user_id, department)
SELECT u.id, 'Default'
  FROM users u
 WHERE NOT ('anonymous' = ANY(u.roles))
   AND u.email NOT LIKE '%@anonymous.local'
   AND NOT EXISTS (SELECT 1 FROM user_profile_ext upe WHERE upe.user_id = u.id)
    ON CONFLICT (user_id) DO NOTHING;
