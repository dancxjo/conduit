# Trusted GitHub App session building block

`tools/ci/pipeline/train-app-session.mjs` provides a scoped installation session
for separately trusted controller code. It is a prerequisite for the dedicated
writer contract in [guard PR #6502](https://github.com/dancxjo/conduit/pull/6502),
not App registration, live installation setup, an activated service, or hosted
train acceptance. No CLI, dispatcher, lander or PR workflow is wired to it here.

The library entry is `withTrainAppSession({config, privateKeyFile, policyApi,
callback, sourceDirectory, transport?, clock?, timeoutMs?})`. The disabled
machine-writer example does not activate it: real App, installation, repository,
ruleset and observer identities must already be reviewed and configured.

## Key and identity custody

The caller supplies one explicit canonical absolute RSA PEM path outside both
the trusted controller and proposed source directories. The helper never searches
for keys. It opens without following symlinks, refuses nonregular or multiply
linked files, requires current-user ownership and owner-only readable permissions,
and bounds the file to 32 KiB. It checks read stability and requires RSA with a
2048–8192 bit modulus. Unsupported ownership platforms and encrypted keys without
a supplied decryption contract refuse. Native key/parser errors are replaced by
public error codes. The helper never rewrites the input key; its read buffer is
cleared after creating the in-memory signing object. This is not a claim of
cryptographic erasure of JavaScript/OpenSSL memory.

The helper signs a real RS256 JWT with `iat` 60 seconds behind the clock, `exp`
nine minutes ahead and `iss` equal to the pinned App ID. It checks authenticated
`GET /app` and installation identity, selected scope, permissions and absence of
event subscriptions before token creation. GitHub permits App ID as issuer,
although client ID is its recommendation. See [JWT generation](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-json-web-token-jwt-for-a-github-app).

The separately supplied `policyApi` authenticates the pinned personal repository
owner and observes the complete ruleset/classic protection. It is the explicit
trusted observer from the writer contract; there is no environment fallback or
credential discovery. The App's administration and checks permissions remain
read-only. Workflow write is retained because exact proved trains can change
workflow files. No extra grants are requested by this helper.

## Scoped issuance and cleanup

The only token-creation request is `POST /app/installations/{configured-id}/access_tokens`,
with exactly one `repository_ids` entry and the complete reviewed permission map.
The result must have an opaque, printable, nonempty token no larger than 16 KiB,
the expected permissions and an unexpired lifetime no longer than one hour.
Token length/prefix is not assumed to be 40 characters: GitHub documents the
new stateless format. See [installation token generation](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-an-installation-access-token-for-a-github-app).

Optional repository fields in the issuance response do not replace actual scope
verification. The minted token must enumerate exactly the configured repository
and report `current_user_can_bypass: "always"` for the configured active ruleset.
The independent full observer policy, source repository identity and classic
proof gate must all validate. The owner identity and credential expiry are checked
again before entering the callback. These checks establish session eligibility;
the existing lander still owns exact-source proof, product custody and ref update
admission on every operation.

A usable opaque token is retained before checking response scope/expiry. Every
subsequent success or refusal reaches one revocation attempt in `finally`, using
`DELETE /installation/token`. There is no retry, refresh or fallback token. If a
token was issued with wrong scope/expiry, callback execution refuses and cleanup
still runs. If transport fails or malformed JSON never provides a trustworthy
token, issuance may be ambiguous: no unsafe token extraction or repeated POST is
attempted, and the callback does not run. The controller must report that boundary
without dumping the response; GitHub's server-side expiry remains the backstop.

`TrainAppSessionError` exposes only a public `code` and `revocation` state:
`not-attempted`, `succeeded` or `failed`. Callback failure stays `callback-failed`
even when cleanup also fails; a successful callback with failed cleanup reports
`revocation-failed`. Original callback, transport, key and JSON error details are
not attached. This allows failure classes to remain distinguishable without
reflecting response bodies or credentials into errors.

## Network and execution boundary

Default transport uses only `https://api.github.com`, fixed configured endpoint
paths, explicit methods, redirect refusal, bounded response bodies (128 KiB) and
deadlines of at most 30 seconds per request. Observer promises also have bounded
waits. No shell, child process, secret environment mutation, stdout, secret argv,
JWT/token file or credential receipt is created. Injected transport/observer and
the clock are test/controller dependencies, not untrusted extension points.

The callback receives `{jwt, installationToken, expiresAt, identity}` in memory
and may return its own noncredential result. It must be pinned trusted controller
code, never proposed PR tooling. It must not print/persist credentials or return
them as user output, and must complete within the token lifetime. JavaScript
cannot preempt a synchronous hostile callback or force it to forget copied
credentials; this helper is not hostile-code confinement or a complete session
supervisor. The next controller integration must preserve separate controller
trust, finite execution ownership, scoped API adapters and the lander's admission
checks without granting secrets to CI proof workers.
Its supported repository entrance must be `cargo xtask` run from the pinned
trusted controller checkout, with proposed source passed explicitly as data.
Never run Cargo or its build scripts from the proposed target checkout while
holding these credentials. This library does not wire that CLI contract itself.

Tests use newly generated ephemeral RSA fixtures, verify signatures using the
actual public key, and exercise lifecycle/refusal/redaction with an injected
transport. They perform no real GitHub token creation, registration or policy
changes. Live App setup, issuer custody, hosted token/refusal/revocation proof,
controller wiring and actual positive train acceptance remain separate work.
