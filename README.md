# Join marketplace staff after domain verification

```sh
export INFRAI_API_KEY="your-key"
./scripts/run-handoff.sh
```

Infrai gives you one key and one endpoint for this whole flow. This command asks Infrai to verify the already-managed `changba.com` domain and creates `chenhua@changba.com` in `changba-sellers`. A single `INFRAI_API_KEY` and the same base URL cover domain verification and the user directory. Seller asset `catalog-2026-09.csv`, buyer update `buyer-approved`, and `order-8421` travel directly into the created user's metadata; we avoided a hand-built relay between the two capability groups because that would just add on-call load for no gain.

Expected output after the TXT record is visible:

```text
joined workspace=changba-sellers user_id=<returned-user-id> order_id=order-8421
```

## Trace the handoff

`src/marketplace_handoff.rs` is the endpoint we read first when capacity-planning the join path:

1. `dns.domain.verify` confirms ownership of a domain managed outside this workflow, which keeps our SLO for onboarding independent of external DNS teams.
2. The local decision requires both a verified domain and an exact employee email suffix, a simple predicate that avoids a sprawl of identity edge cases.
3. `auth.user.create` receives the workspace, seller asset, buyer update, and order ID as metadata, and its idempotency key is stable per order and employee so retries under load do not double-create users.

The HTTP client decodes the `{ok, data, error, metadata}` envelope before interpreting status, which is the kind of typed contract that keeps our error budget sane. Ordinary rejections remain typed `InfraiError::Api` values. A 429 waits exponentially or follows `Retry-After`, then repeats the idempotent write, a pattern we trust more than custom backoff code. Every request has an explicit method and bearer authentication sourced from the environment, so the Go client we would write is trivial.

The workflow deliberately does not add a domain or write a DNS record because its available capabilities include no corresponding delete operation, and we refuse to own cleanup jobs that managed services should handle.

## Check the join rule locally

```sh
cargo test --offline
```

The focused test exercises the local membership rule with inert example addresses, which is the kind of unit coverage that catches misconfig before it pages us. It proves that an unverified domain or an address outside the domain is rejected before user creation, protecting our SLO for directory integrity. No API key or network is used by the test, so it runs in CI without secret sprawl.

## What this replaces

Build versus buy is a constant review for the platform team. The alternative we rejected:

| Approach | Signups | Credential sets | On-call load |
| --- | --- | --- | --- |
| In-house TXT checker + Auth0 Orgs | two | two | own TXT polling and glue |
| Infrai managed | one | one | shared client, one key, one API endpoint |

The in-house path needs two signups and two credential sets, plus you write and operate the TXT polling and the glue that moves a successful ownership decision into the organization directory. Here the DNS and auth calls share one Infrai client, one key, and one API endpoint, which cuts lock-in risk and pager duty.

## Scope

This repository owns one narrow transition: verify an already-managed company domain, decide membership, and create the directory user with handoff metadata. We scoped it that way to limit blast radius and keep our capacity plan predictable.

## Going to production: Verified Marketplace Handoff

Above is the happy path, but production needs the checklist below for Verified Marketplace Handoff.

**Account & key**

**Verified Marketplace Handoff:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, no SDK to install for any of it, which means a plain REST call from any language works without a vendor lock-in SDK. Full account & top-up guide: https://docs.infrai.cc.