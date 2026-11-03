# Escrow: the spec

One escrow between a **buyer**, a **seller** and an **arbiter**. The buyer locks funds; they end up with the seller,
back with the buyer, or split between them by the arbiter. No admin, no owner, nothing can be changed after
creation.

This file and `tests/` are frozen. An implementation is done when `./score.sh` reports every test passing, with
`tests/` unchanged.

## Template

Template name `Escrow`, one component per escrow, in `template/src/lib.rs`, using only `tari_template_lib`.

A party is identified by the **transaction signer's public key** (`CallerContext::transaction_signer_public_key()`).
The buyer is whoever signs the `new` transaction. Epochs are `Consensus::current_epoch()`.

The component allows every method to anyone (`AccessRules::new().default(rule!(allow_all))`) and has no owner
(`OwnerRule::None`); the methods check the signer themselves.

## States

```
open ──dispute──▶ disputed
  │                  │
  ├─release/refund───┤
  ├─reclaim          ├─resolve
  ▼                  ▼
settled ◀────────────┘
```

Once `settled`, the seller's and buyer's shares are fixed and each withdraws their own.

## Methods

| Method | Who | Allowed in | Effect |
|---|---|---|---|
| `new(seller: RistrettoPublicKeyBytes, arbiter: RistrettoPublicKeyBytes, deadline_epoch: u64, funds: Bucket) -> Component<Self>` | anyone (becomes the buyer) | | Locks all of `funds` (any fungible resource). State `open`. |
| `release()` | buyer | open, disputed | Settles: everything to the seller. |
| `refund()` | seller | open, disputed | Settles: everything to the buyer. |
| `dispute()` | buyer or seller | open, before the deadline | State `disputed`. |
| `reclaim()` | buyer | open, at or after the deadline | Settles: everything to the buyer. |
| `resolve(to_seller: Amount)` | arbiter | disputed, any time | Settles: `to_seller` to the seller, the rest to the buyer. |
| `withdraw() -> Bucket` | buyer or seller | settled | Returns the signer's whole share and sets it to zero. |
| `status() -> (String, Amount, Amount, Amount)` | anyone | always | (`"open"`, `"disputed"` or `"settled"`; amount still held; seller's share left; buyer's share left). Shares are zero until settled. |
| `terms() -> (RistrettoPublicKeyBytes, RistrettoPublicKeyBytes, RistrettoPublicKeyBytes, ResourceAddress, Amount, u64)` | anyone | always | (buyer, seller, arbiter, resource, amount locked at creation, deadline epoch). |

"Before the deadline" means `current_epoch < deadline_epoch`; "at or after" is `current_epoch >= deadline_epoch`.
Disputes have no timeout: only the arbiter, or a party giving up (buyer `release`, seller `refund`), ends one.

## Rejections

A rejected call panics with a message **containing** the text below. Checks run in this order, and the first
failure wins: **who is signing → state → epoch → arguments.**

| Text | When |
|---|---|
| `amount must be positive` | `new` with an empty bucket |
| `parties must be distinct` | `new` where any two of buyer, seller and arbiter are the same key |
| `deadline must be in the future` | `new` with `deadline_epoch <= current_epoch` |
| `only the buyer` | `release`, `reclaim` signed by anyone else |
| `only the seller` | `refund` signed by anyone else |
| `only the arbiter` | `resolve` signed by anyone else |
| `only the buyer or seller` | `dispute`, `withdraw` signed by anyone else |
| `escrow is settled` | `release`, `refund`, `dispute`, `reclaim`, `resolve` once settled |
| `escrow is disputed` | `dispute` or `reclaim` while disputed |
| `escrow is not disputed` | `resolve` while open |
| `escrow is not settled` | `withdraw` before settling |
| `deadline has passed` | `dispute` at or after the deadline |
| `deadline has not passed` | `reclaim` before the deadline |
| `share exceeds the escrow` | `resolve` with `to_seller` above the amount held |
| `nothing to withdraw` | `withdraw` when the signer's share is zero |

`new` checks in the order listed: amount, parties, deadline.

## Invariant

Amount held == seller's share left + buyer's share left once settled, and == the amount locked before. Funds only
leave through `withdraw`.

## Units of work

Each test in `tests/tests/escrow.rs` is one unit, prefixed by the method it covers (`new_`, `release_`, `refund_`,
`dispute_`, `reclaim_`, `resolve_`, `withdraw_`). Claim a prefix, make its tests pass without breaking others.
