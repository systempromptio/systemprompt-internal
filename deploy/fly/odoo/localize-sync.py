# Run inside `odoo shell` against a freshly restored copy of the production
# database. Prod rows come with live outgoing mail, live schedulers and real
# user passwords; a dev clone that keeps them mails real customers on its
# first cron tick. Everything here is idempotent.
env['ir.cron'].with_context(active_test=False).search([]).write({'active': False})
print('disabled %s scheduled actions' % env['ir.cron'].with_context(active_test=False).search_count([]))

servers = env['ir.mail_server'].search([])
if servers:
    servers.unlink()
    print('removed outgoing mail servers')

# Belt and braces: even without a server, Odoo can fall back to the host MTA.
env['ir.config_parameter'].sudo().set_param('mail.default.from', 'noreply@localhost')
env['ir.config_parameter'].sudo().set_param('mail.catchall.domain', 'localhost')

# `just e2e-live` and the local demo sign in as admin / admin.
u = env['res.users'].with_context(active_test=False).search([('login', '=', 'admin')], limit=1)
if u:
    u.write({'active': True, 'password': 'admin'})
    print('local admin active. Login: admin / admin.')
else:
    print('WARNING: no `admin` user in the restored database.')

env.cr.commit()
