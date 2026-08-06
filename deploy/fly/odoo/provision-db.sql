-- One-time provisioning of Odoo's database on systemprompt-db-prod.
-- Run as superuser with the password bound: `just odoo-provision-db PW=...`
-- (psql -v pw=... -f provision-db.sql).
--
-- The role owns only its own database; the REVOKEs below close the default
-- PUBLIC CONNECT privilege in both directions (odoo role could otherwise
-- connect to — though not read — site_* databases, and vice versa).
CREATE USER "odoo_88906bfd0afd" WITH PASSWORD :'pw';
CREATE DATABASE "odoo_88906bfd0afd"
  OWNER "odoo_88906bfd0afd"
  ENCODING 'UTF8'
  TEMPLATE template0
  LC_COLLATE 'C'
  LC_CTYPE 'C';
REVOKE CONNECT ON DATABASE "odoo_88906bfd0afd" FROM PUBLIC;
REVOKE CONNECT ON DATABASE "site_88906bfd0afd" FROM PUBLIC;
GRANT CONNECT ON DATABASE "site_88906bfd0afd" TO "tenant_88906bfd0afd";
