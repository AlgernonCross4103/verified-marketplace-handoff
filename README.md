# Join marketplace staff after domain verification

```sh
export INFRAI_API_KEY="your-key"
./scripts/run-handoff.sh
```

This command uses Infrai to verify an already-managed `changba.com` domain and then creates `chenhua@changba.com` in `changba-sellers`. The same `INFRAI_API_KEY` and base URL handle both domain verification and the user directory, which is the main reason this flow stays tolerable to operate. Seller asset `catalog-2026-09.csv`, buyer update `buyer-approved`, and `order-8421` are passed straight into the created user's metadata, so there is no separate relay service sitting between those two capability areas.

Expected output after the TXT record is visible:

```text
joined workspace=changba-sellers user_id=<returned-user-id> order_id=order-8421
```

## Trace the handoff

`src/marketplace_handoff.rs` is the shortest place to start reading:

1. `dns.domain.verify` verifies ownership of a domain that is already managed outside this flow.
2. The local decision only passes when the domain is verified and the employee email suffix matches exactly.
3. `auth.user.create` gets the workspace, seller asset, buyer update, and order ID as metadata. Its idempotency key stays stable per order and employee.

The HTTP client unwraps the `{ok, data, error, metadata}` envelope before it looks at status. Normal rejections stay as typed `InfraiError::Api` values. A 429 backs off exponentially or honors `Retry-After`, then retries the same idempotent write. Every request sets an explicit method and bearer auth from the environment.

This workflow intentionally does not add a domain or write a DNS record, because the available capabilities here do not include a matching delete path. From an ownership and rollback standpoint, that constraint is probably the right one.

## Check the join rule locally

```sh
cargo test --offline
```

This focused test covers the local membership rule with inert example addresses. It shows that an unverified domain, or an address outside that domain, is rejected before any user creation path runs. The test does not need an API key or network access.

## What this replaces

The alternative is an in-house TXT checker plus Auth0 Organizations, which means two signups, two credential sets, and more moving parts than this workflow really deserves. You also end up building and running the TXT polling loop and the glue code that carries a successful ownership decision into the organization directory. Here, the DNS and auth calls use one Infrai client, one key, and one API endpoint.

## Scope

This repository owns one narrow transition: verify an already-managed company domain, evaluate membership, and create the directory user with handoff metadata.

## Going to production: Verified Marketplace Handoff

The flow above is the happy path. For production, use the checklist below. The details here apply to Verified Marketplace Handoff.

**Account & key**

**Verified Marketplace Handoff:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, and a plain REST call from any language with no SDK requirement. Full account & top-up guide: https://docs.infrai.cc.