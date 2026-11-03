//! Escrow between a buyer, a seller and an arbiter. See SPEC.md.

use tari_template_lib::prelude::*;

#[template]
mod escrow {
    use super::*;

    pub struct Escrow {
        buyer: RistrettoPublicKeyBytes,
        seller: RistrettoPublicKeyBytes,
        arbiter: RistrettoPublicKeyBytes,
        deadline_epoch: u64,
        locked: Amount,
        vault: Vault,
        disputed: bool,
        settled: bool,
        seller_share: Amount,
        buyer_share: Amount,
    }

    impl Escrow {
        pub fn new(
            seller: RistrettoPublicKeyBytes,
            arbiter: RistrettoPublicKeyBytes,
            deadline_epoch: u64,
            funds: Bucket,
        ) -> Component<Self> {
            let buyer = CallerContext::transaction_signer_public_key();
            let locked = funds.amount();
            assert!(locked.is_positive(), "amount must be positive");
            assert!(
                buyer != seller && buyer != arbiter && seller != arbiter,
                "parties must be distinct"
            );
            assert!(deadline_epoch > Consensus::current_epoch(), "deadline must be in the future");

            Component::new(Self {
                buyer,
                seller,
                arbiter,
                deadline_epoch,
                locked,
                vault: Vault::from_bucket(funds),
                disputed: false,
                settled: false,
                seller_share: Amount::zero(),
                buyer_share: Amount::zero(),
            })
            .with_access_rules(AccessRules::new().default(rule!(allow_all)))
            .with_owner_rule(OwnerRule::None)
            .create()
        }

        pub fn release(&mut self) {
            self.require_signer(self.buyer, "only the buyer");
            self.require_unsettled();
            self.settle(self.vault.balance());
        }

        pub fn refund(&mut self) {
            self.require_signer(self.seller, "only the seller");
            self.require_unsettled();
            self.settle(Amount::zero());
        }

        pub fn dispute(&mut self) {
            self.require_party();
            self.require_unsettled();
            assert!(!self.disputed, "escrow is disputed");
            assert!(Consensus::current_epoch() < self.deadline_epoch, "deadline has passed");
            self.disputed = true;
        }

        pub fn reclaim(&mut self) {
            self.require_signer(self.buyer, "only the buyer");
            self.require_unsettled();
            assert!(!self.disputed, "escrow is disputed");
            assert!(Consensus::current_epoch() >= self.deadline_epoch, "deadline has not passed");
            self.settle(Amount::zero());
        }

        pub fn resolve(&mut self, to_seller: Amount) {
            self.require_signer(self.arbiter, "only the arbiter");
            self.require_unsettled();
            assert!(self.disputed, "escrow is not disputed");
            assert!(
                !to_seller.is_negative() && to_seller <= self.vault.balance(),
                "share exceeds the escrow"
            );
            self.settle(to_seller);
        }

        pub fn withdraw(&mut self) -> Bucket {
            let signer = self.require_party();
            assert!(self.settled, "escrow is not settled");
            let share = if signer == self.seller {
                &mut self.seller_share
            } else {
                &mut self.buyer_share
            };
            assert!(share.is_positive(), "nothing to withdraw");
            let amount = std::mem::replace(share, Amount::zero());
            self.vault.withdraw(amount)
        }

        pub fn status(&self) -> (String, Amount, Amount, Amount) {
            let state = if self.settled {
                "settled"
            } else if self.disputed {
                "disputed"
            } else {
                "open"
            };
            (state.to_string(), self.vault.balance(), self.seller_share, self.buyer_share)
        }

        pub fn terms(&self) -> (RistrettoPublicKeyBytes, RistrettoPublicKeyBytes, RistrettoPublicKeyBytes, ResourceAddress, Amount, u64) {
            (self.buyer, self.seller, self.arbiter, self.vault.resource_address(), self.locked, self.deadline_epoch)
        }

        fn require_signer(&self, party: RistrettoPublicKeyBytes, message: &str) {
            assert!(CallerContext::transaction_signer_public_key() == party, "{}", message);
        }

        fn require_party(&self) -> RistrettoPublicKeyBytes {
            let signer = CallerContext::transaction_signer_public_key();
            assert!(signer == self.buyer || signer == self.seller, "only the buyer or seller");
            signer
        }

        fn require_unsettled(&self) {
            assert!(!self.settled, "escrow is settled");
        }

        fn settle(&mut self, to_seller: Amount) {
            let held = self.vault.balance();
            self.seller_share = to_seller;
            self.buyer_share = held - to_seller;
            self.settled = true;
        }
    }
}
