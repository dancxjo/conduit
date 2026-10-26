# Dedicated development train writer

This follow-up is an inactive setup contract. No GitHub App has been registered,
no real App/installation/ruleset identity is supplied, and no live policy has
been installed by this change. The example configuration deliberately refuses
activation. It does not enable global cheap PR admission.

The earlier train implementation requires exact-source exhaustive Integration
proof and verifies its artifacts before a non-forced development update. Requiring
`train-accepted` from Actions distinguishes that proof from ordinary Candidate
success, but does not make the writer machine-exclusive: another authorized
writer can update the same checked commit outside the lander. This follow-up
adds a separate writer restriction without bypassing proof protection.

## Two independent server gates

Classic `dev` protection must require only `train-accepted` from Actions app
15368, enforce it for administrators, require no PR, prohibit force pushes and
deletion, and contain no unrecognized requirement or classic push restriction.
Its purpose is proof, not writer identity.

A separate repository branch ruleset named `conduit-dev-machine-writer` must
be active, sourced from this exact repository, and match exactly
`refs/heads/dev`, with no exclusions or other conditions. It contains exactly:

```json
{
  "type": "update",
  "parameters": { "update_allows_fetch_and_merge": false }
}
```

Its only bypass actor must be the separately reviewed dedicated GitHub App ID,
with `actor_type: "Integration"` and `bypass_mode: "always"`. The App bypasses
only this update rule. Required checks must never be put in this bypassed
ruleset. No administrator, repository role, human, team, deploy key, second App,
pull-request-only bypass, or exempt bypass is admitted. The effective branch
rules API must report exactly this repository update rule and reviewed ruleset
ID. Disabled/evaluate rulesets, broad patterns, inherited rules and additional
applicable rules refuse.

This repository is personally owned. Classic actor push restrictions are an
organization-owned repository feature; branch rulesets provide the separate
update restriction here. GitHub documents that only bypass actors may update
matching refs. See [available rules](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets)
and [protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches).
Repository administrators can still edit policy itself; this is not a claim to
contain the repository owner or a compromised App custodian.

## Review before registration and installation

The reviewable [App manifest](../../tools/ci/pipeline/train-app-manifest.json)
requests contents:write for train refs, actions:write for dispatch, checks:read,
pull_requests:read, metadata:read and administration:read for observation.
Administration write, check/status write, workflow write and other grants are
absent. The App is private, requests no user OAuth grant, subscribes to no
events, and disables webhook delivery. The inactive hook URL names the
repository, not a deployed webhook service. If webhooks become necessary, they
require their own reviewed service/secret contract before activation.

Registration is a separate operator action. The [manifest flow](https://docs.github.com/en/apps/sharing-github-apps/registering-a-github-app-from-a-manifest)
creates credentials, so do not run it from a PR job or log its response. Select
only `dancxjo/conduit` during installation. Store the private key, client secrets
and generated tokens outside Git and outside PR-controlled execution. This
change neither generates nor stores them.

Copy [the disabled identity example](../../tools/ci/pipeline/train-machine-writer.example.json)
to an absolute external file. Fill repository ID, actual App ID/slug, selected
installation ID and actual ruleset ID from separately authenticated observations.
The fixture identities in tests are invented and must never be used for setup.
GitHub Actions app15368, a connector's unexplained installation ID, a bot
username and a human PAT are not substitutes for this dedicated App identity.
Enable the external configuration only after the two live gates and the App
installation have been reviewed and read back exactly.

## Authentication and supported landing

The trusted controller supplies two independent existing credentials:

- `GH_TOKEN`: a short-lived installation token restricted to the selected
  repository and the reviewed permissions; the installation-only repositories
  endpoint must report exactly the expected repository ID/name.
- A separately named environment variable containing a short-lived App JWT:
  it is used only for `GET /app` and `GET /app/installations/{id}`. App JWTs and
  installation tokens are distinct; no minting is implemented here.

[App identity endpoints](https://docs.github.com/en/rest/apps/apps#get-the-authenticated-app)
require App JWT authentication. [Installation repositories](https://docs.github.com/en/rest/apps/installations#list-repositories-accessible-to-the-app-installation)
uses installation authentication. A human `gh` session cannot silently replace
these channels. Classic protection observation requires Administration:read,
which ordinary workflow `GITHUB_TOKEN` permissions do not offer. Never pass an
administrative or App private credential into PR-authored scripts.

After the separate setup and activation, the supported entrance is:

```sh
cargo xtask ci pipeline train-land /absolute/train.json INTEGRATION_RUN_ID /absolute/archives /absolute/new-landing /absolute/reviewed-machine-policy.json CONDUIT_TRAIN_APP_JWT
```

The final argument is a variable name, never the JWT value. The external JSON
contains public reviewed identities, never credentials. The command does not
register/install an App, mint tokens or edit protection/rulesets. Missing
identities, channels, permissions or live rules refuse. App/installation identity
and both server gates are observed before and after product restoration. Their
public identities are retained in the landing intent and result. A server
rejection or ambiguous write remains terminal, with no forced update, human
fallback or mutation retry.

The identity observations do not cryptographically pair an arbitrary supplied
installation token with the JWT before a write: the installation-only API
rejects human tokens, and the independent server update rule ultimately permits
only the configured App. A token from another installed App must be denied by
that server gate. Keep token issuance in the trusted App controller; live
acceptance must include wrong-token refusal and actual correct-App landing.

App-triggered pushes can emit the dev push Integration needed for adoption;
ordinary `GITHUB_TOKEN` pushes suppress push-triggered runs. A manual dispatch
is not equivalent to accepted dev push proof. If the minimal manifest cannot
perform an actual supported ref update for a workflow-changing train, stop and
review the exact required permission separately; do not add privileges or switch
to a human token automatically.

## Activation proof still required

Bootstrap the trusted code through the old ordinary PR route while its existing
checks remain intact. Capture old protection/rules and an explicit reviewed
transition outside Git. Register/install the real App and provision its trusted
credential channels separately. Apply the narrow update ruleset and independent
proof protection with operator authority, then read both back. Do not make PR
checks cheaper until the enforced writer/proof contract has positive live proof.

Required hosted acceptance includes: the wrong actor cannot update even a green
commit; the configured App cannot update without `train-accepted`; unknown
policy or stale identity refuses before mutation; the App lands only the exact
restored source without force; actual dev push Integration adopts the original
proof; publication authenticates the same products. Fixtures establish validator
behavior only. Live activation, App setup, positive landing and global cheap
admission remain unfinished until those distinct boundaries pass.
