# Composing a kit on another instance

A kit is published once, by its repository's CI, as a signed services bundle
on GHCR. Any systemprompt instance built from this repository can compose it;
nothing in the kit is instance-specific. This is what an operator of a
separate deployment does to follow the same kits as the primary instance. The
model is `/documentation/services-sync`; the kit contract is
`deploy/kit/README-INTEGRATION.md`; the kits this repository knows are listed
in `deploy/kit/known-kits.json` (none yet).

## 1. A pull token for private packages

A kit whose GHCR package is private (`pull: private` in `known-kits.json`) is
pulled with one secret:

| secret | value |
|---|---|
| `ghcr_pull_token` | `<github-user>:<PAT>` — a GitHub personal access token (classic) with the single scope `read:packages`, on an account that can read the kit owner's packages |

Put it in the profile's `secrets.json` (or the equivalent secret store the
deployment uses; `secrets.source` in the profile says which). A public kit
needs no secret and its source entry carries no `auth_secret`.

## 2. The sources block

Add to the instance's profile (`.systemprompt/profiles/<name>/profile.yaml`):

```yaml
services:
  sources:
    - name: <kit>
      oci:
        reference: ghcr.io/<owner>/<kit-repository>:stable
        auth_secret: ghcr_pull_token          # private packages only
        verify:
          ed25519_public_keys: ["<the kit's public key>"]
  cache_dir: /app/services-cache        # any writable directory
  on_fetch_failure: use_last_good       # use_bundled on the very first boot
```

The public key is the one recorded for the kit in `deploy/kit/known-kits.json`;
a bundle signed by any other key is refused. `just services-pin <kit> stable
<profile>` writes exactly this entry.

## 3. Entitlement stays in this repository

Who reaches a kit's marketplace is declared in
`services/access-control/rules.yaml` as `marketplace/<id>` with
`owner: bundle:<kit>`, and any group or project it names is declared in
`services/web/config/groups.yaml`. An instance deployed from this repository
carries both, so a kit never decides its own audience.

## 4. Import after every kit release

A kit release moves its `stable` tag. The instance serves it after one
**Import sources** on `/admin/sync`, or:

```bash
curl -fsS -X POST -H "Authorization: Bearer <admin PAT>" \
  https://<instance>/api/v1/admin/services/refresh
# {"changed":true,"reconciled":true,"restart_recommended":false,...}
```

Nothing restarts. The kit CI can make this call itself for **one** instance
(its `SYSTEMPROMPT_API_URL` / `SYSTEMPROMPT_ADMIN_TOKEN` secrets); every other
instance imports on its own schedule. `GET /api/v1/admin/services/status`
shows the active digest per source.

## 5. Rollback

`just services-pin <kit> sha256:<previous digest> <profile>` and Import; the
cache keeps the last two trees per source. The digests are in each kit's
release run summary and on the GHCR package page.
