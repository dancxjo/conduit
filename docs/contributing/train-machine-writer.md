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
It also requests workflows:write, because exact proved train trees can change
workflow files: the [ref update API](https://docs.github.com/en/rest/git/refs#update-a-reference)
documents that permission alongside contents:write for those updates. This is
authority to land the exact authenticated tree, not permission for PR code to
use credentials. Administration write, check/status write and unrelated grants
are absent. The App is private, requests no user OAuth grant, subscribes to no
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
Pin the personal repository owner's actual login and numeric user ID for the
separate trusted policy observer; an arbitrary administrator or role is not
accepted by this narrow contract.
The fixture identities in tests are invented and must never be used for setup.
GitHub Actions app15368, a connector's unexplained installation ID, a bot
username and a human PAT are not substitutes for this dedicated App identity.
Enable the external configuration only after the two live gates and the App
installation have been reviewed and read back exactly.

## Authentication and supported landing

The trusted controller supplies three independent existing credentials:

- `GH_TOKEN`: a short-lived installation token restricted to the selected
  repository and the reviewed permissions; the installation-only repositories
  endpoint must report exactly the expected repository ID/name.
- A separately named environment variable containing a short-lived App JWT:
  it is used only for `GET /app` and `GET /app/installations/{id}`. App JWTs and
  installation tokens are distinct; no minting is implemented here.
- A different named environment variable containing the trusted policy observer
  credential. Its `GET /user` response must match the pinned personal repository
  owner login and numeric ID. It must have visibility of the complete ruleset
  bypass list. Its client can only GET that identity, the exact configured
  ruleset, dev protection, dev branch rules and the exact
  `CONDUIT_PR_ADMISSION_TIER` repository variable. It cannot mutate policy or
  refs, enumerate other variables or substitute for the installation token.

[App identity endpoints](https://docs.github.com/en/rest/apps/apps#get-the-authenticated-app)
require App JWT authentication. [Installation repositories](https://docs.github.com/en/rest/apps/installations#list-repositories-accessible-to-the-app-installation)
uses installation authentication. A human `gh` session cannot silently replace
these channels. Classic protection observation requires Administration:read,
which ordinary workflow `GITHUB_TOKEN` permissions do not offer. Never pass an
administrative or App private credential into PR-authored scripts.

GitHub [ruleset observation](https://docs.github.com/en/rest/repos/rules#get-a-repository-ruleset)
can omit `bypass_actors` unless the requesting actor has write access to the
ruleset. Administration:read on the App is not treated as a guarantee of full
bypass visibility, and is never escalated to write. The separately supplied
owner observer must obtain the complete list; an omitted list refuses.
Ordinary branch-rule enumeration reports active rules independently of their
source and excludes evaluate/disabled rulesets, so the specific ruleset's
conditions/enforcement are checked as well.

The installation token independently reads the configured repository ruleset
and must receive the documented `current_user_can_bypass: "always"` value.
That request-specific field is a snapshot of the mutation token's effective
bypass, checked against the owner-observed sole-App policy. Missing, never,
PR-only or exempt values refuse. It is not inferred from the JWT, a role name,
or a public App identity. Live acceptance must capture these actual responses.

After setup and activation, run the privileged entrance from the clean proposed
source working directory, executing only a separately pinned trusted controller:

```sh
node /absolute/trusted-controller/tools/ci/pipeline/cli.mjs train-land /absolute/train.json INTEGRATION_RUN_ID /absolute/archives /absolute/new-landing /absolute/reviewed-machine-policy.json CONDUIT_TRAIN_APP_JWT CONDUIT_TRAIN_POLICY_OBSERVER
```

The final two arguments are distinct variable names, never credential values;
neither may be `GH_TOKEN` or `GITHUB_TOKEN`. The external JSON
contains public reviewed identities, never credentials. The command does not
register/install an App, mint tokens or edit protection/rulesets. Missing
identities, channels, permissions or live rules refuse. App/installation identity
and observer identities and both server gates are observed before and after product restoration. Their
public identities are retained in the landing intent and result. A server
rejection or ambiguous write remains terminal, with no forced update, human
fallback or mutation retry.

The command refuses controller and proposed-source directories that coincide
or contain each other, including through symlinks. That path fence does not
establish trust: the operator must separately review and pin the complete
controller tree and its runtime before supplying credentials. Source assertions
check Git/data identity only. Never invoke proposed PR Cargo/xtask scripts or its
copy of the CLI with these credentials. Module imports resolve from the trusted
controller while data/Git operations use the proposed source working directory.
An issuer/session controller and its authenticated trust handoff are still
separate setup work; this change does not provide them.

The installation-only repositories API rejects human tokens. The independent
server update gate is the final actor enforcement in addition to the observed
request-specific bypass. Keep token issuance in the trusted App controller;
live acceptance must include wrong-token refusal and actual correct-App landing.

App-triggered pushes can emit the dev push Integration needed for adoption;
ordinary `GITHUB_TOKEN` pushes suppress push-triggered runs. A manual dispatch
is not equivalent to accepted dev push proof. No identity channel can mint tokens,
register the App, or switch to another credential automatically.

## Activation proof still required

Compatibility remains intentionally fail-closed. GitHub's
[official OpenAPI schema](https://github.com/github/rest-api-description/blob/main/descriptions/api.github.com/api.github.com.json)
makes an update rule's `parameters` object optional, with
`update_allows_fetch_and_merge` required if that object exists. This validator
currently requires an explicit false value in both rule observations; a valid
API response omitting parameters will refuse and needs a separately reviewed
compatibility decision based on actual policy readback. The App response schema
also does not require `slug`, although documented registered-App examples include
it; this contract requires the reviewed slug rather than inferring identity.
Fixture success does not establish those live response shapes.

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
