//! Escrow between a buyer, a seller and an arbiter. See SPEC.md; this stub fails every test.

use tari_template_lib::prelude::*;

#[template]
mod escrow {
    use super::*;

    pub struct Escrow {}

    impl Escrow {
        pub fn new(_seller: RistrettoPublicKeyBytes, _arbiter: RistrettoPublicKeyBytes, _deadline_epoch: u64, _funds: Bucket) -> Component<Self> {
            panic!("not implemented")
        }

        pub fn release(&mut self) {
            panic!("not implemented")
        }

        pub fn refund(&mut self) {
            panic!("not implemented")
        }

        pub fn dispute(&mut self) {
            panic!("not implemented")
        }

        pub fn reclaim(&mut self) {
            panic!("not implemented")
        }

        pub fn resolve(&mut self, _to_seller: Amount) {
            panic!("not implemented")
        }

        pub fn withdraw(&mut self) -> Bucket {
            panic!("not implemented")
        }

        pub fn status(&self) -> (String, Amount, Amount, Amount) {
            panic!("not implemented")
        }

        pub fn terms(&self) -> (RistrettoPublicKeyBytes, RistrettoPublicKeyBytes, RistrettoPublicKeyBytes, ResourceAddress, Amount, u64) {
            panic!("not implemented")
        }
    }
}
