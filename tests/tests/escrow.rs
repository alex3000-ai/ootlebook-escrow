//! The Escrow pilot's oracle: each test is one unit of SPEC.md, run in-process by the Ootle engine.

use ootle_byte_type::ToByteType;
use tari_crypto::{
    keys::PublicKey as _,
    ristretto::{RistrettoPublicKey, RistrettoSecretKey},
};
use tari_ootle_transaction::{args, builder::named_args::NamedArg, Transaction};
use tari_template_lib_types::{
    constants::TARI_TOKEN, crypto::RistrettoPublicKeyBytes, Amount, ComponentAddress, NonFungibleAddress, ResourceAddress,
};
use tari_engine_types::virtual_substate::{VirtualSubstate, VirtualSubstateId};
use tari_template_test_tooling::TemplateTest;

const AMOUNT: u64 = 1_000;
const DEADLINE: u64 = 10;

#[derive(Clone, Copy, Debug)]
enum Role {
    Buyer,
    Seller,
    Arbiter,
    Stranger,
}
use Role::*;

struct Party {
    account: ComponentAddress,
    proof: NonFungibleAddress,
    key: RistrettoSecretKey,
}

struct Deal {
    test: TemplateTest,
    parties: [Party; 4],
    escrow: Option<ComponentAddress>,
}

type Status = (String, Amount, Amount, Amount);

fn status(state: &str, held: u64, seller: u64, buyer: u64) -> Status {
    (state.into(), held.into(), seller.into(), buyer.into())
}

impl Deal {
    /// Four funded accounts and no escrow yet.
    fn parties() -> Self {
        let mut test = TemplateTest::new(env!("CARGO_MANIFEST_DIR"), ["../template"]);
        let parties = [(); 4].map(|_| {
            let (account, proof, key) = test.create_funded_account();
            Party { account, proof, key }
        });
        Self { test, parties, escrow: None }
    }

    /// An open escrow of AMOUNT with deadline DEADLINE, created at epoch 0.
    fn open() -> Self {
        let mut d = Self::parties();
        let tx = d.create_tx(d.pk(Seller), d.pk(Arbiter), DEADLINE, AMOUNT);
        d.ok(Buyer, tx);
        d.escrow = d.test.return_values().iter().find_map(|v| v.component_addresses().first().copied());
        d
    }

    fn party(&self, r: Role) -> &Party {
        &self.parties[r as usize]
    }

    fn pk(&self, r: Role) -> RistrettoPublicKeyBytes {
        RistrettoPublicKey::from_secret_key(&self.party(r).key).to_byte_type()
    }

    fn escrow(&self) -> ComponentAddress {
        self.escrow.expect("escrow created")
    }

    fn create_tx(&self, seller: RistrettoPublicKeyBytes, arbiter: RistrettoPublicKeyBytes, deadline: u64, amount: u64) -> Transaction {
        let buyer = self.party(Buyer);
        let template = self.test.get_template_address("Escrow");
        self.test
            .transaction()
            .call_method(buyer.account, "withdraw", args![TARI_TOKEN, Amount::from(amount)])
            .put_last_instruction_output_on_workspace("funds")
            .call_function(template, "new", args![seller, arbiter, deadline, Workspace("funds")])
            .finish()
            .seal(&buyer.key)
    }

    fn create_fails(&mut self, seller: RistrettoPublicKeyBytes, arbiter: RistrettoPublicKeyBytes, deadline: u64, amount: u64, expected: &str) {
        let tx = self.create_tx(seller, arbiter, deadline, amount);
        self.fails(Buyer, tx, expected);
    }

    fn tx(&self, r: Role, method: &str, args: Vec<NamedArg>) -> Transaction {
        self.test.transaction().call_method(self.escrow(), method, args).finish().seal(&self.party(r).key)
    }

    fn withdraw_tx(&self, r: Role) -> Transaction {
        let p = self.party(r);
        self.test
            .transaction()
            .call_method(self.escrow(), "withdraw", args![])
            .put_last_instruction_output_on_workspace("out")
            .call_method(p.account, "deposit", args![Workspace("out")])
            .finish()
            .seal(&p.key)
    }

    fn ok(&mut self, r: Role, tx: Transaction) {
        let proof = self.party(r).proof.clone();
        self.test.execute_expect_success(tx, vec![proof]);
    }

    fn fails(&mut self, r: Role, tx: Transaction, expected: &str) {
        let proof = self.party(r).proof.clone();
        let reason = self.test.execute_expect_failure(tx, vec![proof]).to_string();
        assert!(reason.contains(expected), "{r:?}: expected rejection containing {expected:?}, got: {reason}");
    }

    fn call(&mut self, r: Role, method: &str, args: Vec<NamedArg>) {
        let tx = self.tx(r, method, args);
        self.ok(r, tx);
    }

    fn call_fails(&mut self, r: Role, method: &str, args: Vec<NamedArg>, expected: &str) {
        let tx = self.tx(r, method, args);
        self.fails(r, tx, expected);
    }

    fn withdraw(&mut self, r: Role) {
        let tx = self.withdraw_tx(r);
        self.ok(r, tx);
    }

    fn withdraw_fails(&mut self, r: Role, expected: &str) {
        let tx = self.withdraw_tx(r);
        self.fails(r, tx, expected);
    }

    fn status(&mut self) -> Status {
        self.test.call_method(self.escrow(), "status", args![], vec![])
    }

    fn balance(&mut self, r: Role) -> Amount {
        let account = self.party(r).account;
        self.test.call_method(account, "balance", args![TARI_TOKEN], vec![])
    }

    fn set_epoch(&mut self, epoch: u64) {
        self.test.set_virtual_substate(VirtualSubstateId::CurrentEpoch, VirtualSubstate::CurrentEpoch(epoch));
    }

    fn dispute(&mut self) -> &mut Self {
        self.call(Buyer, "dispute", args![]);
        self
    }
}

// ---- new ----

#[test]
fn new_records_terms_and_locks_the_funds() {
    let mut d = Deal::parties();
    let before = d.balance(Buyer);
    let tx = d.create_tx(d.pk(Seller), d.pk(Arbiter), DEADLINE, AMOUNT);
    d.ok(Buyer, tx);
    d.escrow = d.test.return_values().iter().find_map(|v| v.component_addresses().first().copied());
    assert_eq!(d.balance(Buyer), before - Amount::from(AMOUNT));
    assert_eq!(d.status(), status("open", AMOUNT, 0, 0));
    let terms: (RistrettoPublicKeyBytes, RistrettoPublicKeyBytes, RistrettoPublicKeyBytes, ResourceAddress, Amount, u64) =
        d.test.call_method(d.escrow(), "terms", args![], vec![]);
    assert_eq!(terms, (d.pk(Buyer), d.pk(Seller), d.pk(Arbiter), TARI_TOKEN, AMOUNT.into(), DEADLINE));
}

#[test]
fn new_rejects_an_empty_bucket() {
    let mut d = Deal::parties();
    d.create_fails(d.pk(Seller), d.pk(Arbiter), DEADLINE, 0, "amount must be positive");
}

#[test]
fn new_rejects_the_buyer_as_seller() {
    let mut d = Deal::parties();
    d.create_fails(d.pk(Buyer), d.pk(Arbiter), DEADLINE, AMOUNT, "parties must be distinct");
}

#[test]
fn new_rejects_the_buyer_as_arbiter() {
    let mut d = Deal::parties();
    d.create_fails(d.pk(Seller), d.pk(Buyer), DEADLINE, AMOUNT, "parties must be distinct");
}

#[test]
fn new_rejects_the_seller_as_arbiter() {
    let mut d = Deal::parties();
    d.create_fails(d.pk(Seller), d.pk(Seller), DEADLINE, AMOUNT, "parties must be distinct");
}

#[test]
fn new_rejects_a_deadline_of_now() {
    let mut d = Deal::parties();
    d.set_epoch(5);
    d.create_fails(d.pk(Seller), d.pk(Arbiter), 5, AMOUNT, "deadline must be in the future");
}

#[test]
fn new_rejects_a_past_deadline() {
    let mut d = Deal::parties();
    d.set_epoch(5);
    d.create_fails(d.pk(Seller), d.pk(Arbiter), 3, AMOUNT, "deadline must be in the future");
}

#[test]
fn new_checks_the_amount_before_the_parties() {
    let mut d = Deal::parties();
    d.create_fails(d.pk(Buyer), d.pk(Buyer), DEADLINE, 0, "amount must be positive");
}

#[test]
fn new_checks_the_parties_before_the_deadline() {
    let mut d = Deal::parties();
    d.create_fails(d.pk(Buyer), d.pk(Arbiter), 0, AMOUNT, "parties must be distinct");
}

// ---- release ----

#[test]
fn release_settles_everything_to_the_seller() {
    let mut d = Deal::open();
    d.call(Buyer, "release", args![]);
    assert_eq!(d.status(), status("settled", AMOUNT, AMOUNT, 0));
}

#[test]
fn release_is_only_for_the_buyer() {
    let mut d = Deal::open();
    for r in [Seller, Arbiter, Stranger] {
        d.call_fails(r, "release", args![], "only the buyer");
    }
}

#[test]
fn release_ends_a_dispute() {
    let mut d = Deal::open();
    d.dispute().call(Buyer, "release", args![]);
    assert_eq!(d.status(), status("settled", AMOUNT, AMOUNT, 0));
}

#[test]
fn release_works_after_the_deadline() {
    let mut d = Deal::open();
    d.set_epoch(DEADLINE + 5);
    d.call(Buyer, "release", args![]);
    assert_eq!(d.status(), status("settled", AMOUNT, AMOUNT, 0));
}

#[test]
fn release_fails_once_settled() {
    let mut d = Deal::open();
    d.call(Seller, "refund", args![]);
    d.call_fails(Buyer, "release", args![], "escrow is settled");
}

// ---- refund ----

#[test]
fn refund_settles_everything_to_the_buyer() {
    let mut d = Deal::open();
    d.call(Seller, "refund", args![]);
    assert_eq!(d.status(), status("settled", AMOUNT, 0, AMOUNT));
}

#[test]
fn refund_is_only_for_the_seller() {
    let mut d = Deal::open();
    for r in [Buyer, Arbiter, Stranger] {
        d.call_fails(r, "refund", args![], "only the seller");
    }
}

#[test]
fn refund_ends_a_dispute() {
    let mut d = Deal::open();
    d.dispute().call(Seller, "refund", args![]);
    assert_eq!(d.status(), status("settled", AMOUNT, 0, AMOUNT));
}

#[test]
fn refund_fails_once_settled() {
    let mut d = Deal::open();
    d.call(Buyer, "release", args![]);
    d.call_fails(Seller, "refund", args![], "escrow is settled");
}

// ---- dispute ----

#[test]
fn dispute_by_the_buyer() {
    let mut d = Deal::open();
    d.call(Buyer, "dispute", args![]);
    assert_eq!(d.status(), status("disputed", AMOUNT, 0, 0));
}

#[test]
fn dispute_by_the_seller() {
    let mut d = Deal::open();
    d.call(Seller, "dispute", args![]);
    assert_eq!(d.status(), status("disputed", AMOUNT, 0, 0));
}

#[test]
fn dispute_is_only_for_buyer_or_seller() {
    let mut d = Deal::open();
    for r in [Arbiter, Stranger] {
        d.call_fails(r, "dispute", args![], "only the buyer or seller");
    }
}

#[test]
fn dispute_fails_while_disputed() {
    let mut d = Deal::open();
    d.dispute().call_fails(Seller, "dispute", args![], "escrow is disputed");
}

#[test]
fn dispute_works_just_before_the_deadline() {
    let mut d = Deal::open();
    d.set_epoch(DEADLINE - 1);
    d.call(Seller, "dispute", args![]);
    assert_eq!(d.status().0, "disputed");
}

#[test]
fn dispute_fails_at_the_deadline() {
    let mut d = Deal::open();
    d.set_epoch(DEADLINE);
    d.call_fails(Seller, "dispute", args![], "deadline has passed");
}

#[test]
fn dispute_fails_once_settled() {
    let mut d = Deal::open();
    d.call(Buyer, "release", args![]);
    d.call_fails(Seller, "dispute", args![], "escrow is settled");
}

#[test]
fn dispute_checks_the_state_before_the_epoch() {
    let mut d = Deal::open();
    d.call(Buyer, "release", args![]);
    d.set_epoch(DEADLINE);
    d.call_fails(Seller, "dispute", args![], "escrow is settled");
}

// ---- reclaim ----

#[test]
fn reclaim_at_the_deadline_settles_to_the_buyer() {
    let mut d = Deal::open();
    d.set_epoch(DEADLINE);
    d.call(Buyer, "reclaim", args![]);
    assert_eq!(d.status(), status("settled", AMOUNT, 0, AMOUNT));
}

#[test]
fn reclaim_fails_before_the_deadline() {
    let mut d = Deal::open();
    d.set_epoch(DEADLINE - 1);
    d.call_fails(Buyer, "reclaim", args![], "deadline has not passed");
}

#[test]
fn reclaim_is_only_for_the_buyer() {
    let mut d = Deal::open();
    d.set_epoch(DEADLINE);
    for r in [Seller, Arbiter, Stranger] {
        d.call_fails(r, "reclaim", args![], "only the buyer");
    }
}

#[test]
fn reclaim_fails_while_disputed() {
    let mut d = Deal::open();
    d.dispute().set_epoch(DEADLINE + 100);
    d.call_fails(Buyer, "reclaim", args![], "escrow is disputed");
}

#[test]
fn reclaim_fails_once_settled() {
    let mut d = Deal::open();
    d.call(Buyer, "release", args![]);
    d.set_epoch(DEADLINE);
    d.call_fails(Buyer, "reclaim", args![], "escrow is settled");
}

#[test]
fn reclaim_checks_the_signer_first() {
    let mut d = Deal::open();
    d.call_fails(Stranger, "reclaim", args![], "only the buyer");
}

// ---- resolve ----

#[test]
fn resolve_splits_the_escrow() {
    let mut d = Deal::open();
    d.dispute().call(Arbiter, "resolve", args![Amount::from(300u64)]);
    assert_eq!(d.status(), status("settled", AMOUNT, 300, AMOUNT - 300));
}

#[test]
fn resolve_can_give_everything_to_the_seller() {
    let mut d = Deal::open();
    d.dispute().call(Arbiter, "resolve", args![Amount::from(AMOUNT)]);
    assert_eq!(d.status(), status("settled", AMOUNT, AMOUNT, 0));
}

#[test]
fn resolve_can_give_everything_to_the_buyer() {
    let mut d = Deal::open();
    d.dispute().call(Arbiter, "resolve", args![Amount::zero()]);
    assert_eq!(d.status(), status("settled", AMOUNT, 0, AMOUNT));
}

#[test]
fn resolve_is_only_for_the_arbiter() {
    let mut d = Deal::open();
    d.dispute();
    for r in [Buyer, Seller, Stranger] {
        d.call_fails(r, "resolve", args![Amount::from(1u64)], "only the arbiter");
    }
}

#[test]
fn resolve_fails_without_a_dispute() {
    let mut d = Deal::open();
    d.call_fails(Arbiter, "resolve", args![Amount::from(1u64)], "escrow is not disputed");
}

#[test]
fn resolve_rejects_more_than_the_escrow() {
    let mut d = Deal::open();
    d.dispute().call_fails(Arbiter, "resolve", args![Amount::from(AMOUNT + 1)], "share exceeds the escrow");
}

#[test]
fn resolve_fails_once_settled() {
    let mut d = Deal::open();
    d.dispute().call(Buyer, "release", args![]);
    d.call_fails(Arbiter, "resolve", args![Amount::from(1u64)], "escrow is settled");
}

#[test]
fn resolve_has_no_deadline() {
    let mut d = Deal::open();
    d.dispute().set_epoch(DEADLINE + 1_000);
    d.call(Arbiter, "resolve", args![Amount::from(1u64)]);
    assert_eq!(d.status(), status("settled", AMOUNT, 1, AMOUNT - 1));
}

// ---- withdraw ----

#[test]
fn withdraw_pays_the_seller_after_release() {
    let mut d = Deal::open();
    let before = d.balance(Seller);
    d.call(Buyer, "release", args![]);
    d.withdraw(Seller);
    assert_eq!(d.balance(Seller), before + Amount::from(AMOUNT));
    assert_eq!(d.status(), status("settled", 0, 0, 0));
}

#[test]
fn withdraw_pays_the_buyer_after_reclaim() {
    let mut d = Deal::open();
    let before = d.balance(Buyer);
    d.set_epoch(DEADLINE);
    d.call(Buyer, "reclaim", args![]);
    d.withdraw(Buyer);
    assert_eq!(d.balance(Buyer), before + Amount::from(AMOUNT));
    assert_eq!(d.status(), status("settled", 0, 0, 0));
}

#[test]
fn withdraw_pays_each_side_its_share() {
    let mut d = Deal::open();
    let (buyer, seller) = (d.balance(Buyer), d.balance(Seller));
    d.dispute().call(Arbiter, "resolve", args![Amount::from(300u64)]);
    d.withdraw(Seller);
    assert_eq!(d.status(), status("settled", AMOUNT - 300, 0, AMOUNT - 300));
    d.withdraw(Buyer);
    assert_eq!(d.status(), status("settled", 0, 0, 0));
    assert_eq!(d.balance(Seller), seller + Amount::from(300u64));
    assert_eq!(d.balance(Buyer), buyer + Amount::from(AMOUNT - 300));
}

#[test]
fn withdraw_fails_before_settling() {
    let mut d = Deal::open();
    d.withdraw_fails(Seller, "escrow is not settled");
    d.dispute().withdraw_fails(Buyer, "escrow is not settled");
}

#[test]
fn withdraw_fails_the_second_time() {
    let mut d = Deal::open();
    d.call(Buyer, "release", args![]);
    d.withdraw(Seller);
    d.withdraw_fails(Seller, "nothing to withdraw");
}

#[test]
fn withdraw_fails_with_no_share() {
    let mut d = Deal::open();
    d.call(Buyer, "release", args![]);
    d.withdraw_fails(Buyer, "nothing to withdraw");
}

#[test]
fn withdraw_is_only_for_buyer_or_seller() {
    let mut d = Deal::open();
    d.call(Buyer, "release", args![]);
    for r in [Arbiter, Stranger] {
        d.withdraw_fails(r, "only the buyer or seller");
    }
    assert_eq!(d.status(), status("settled", AMOUNT, AMOUNT, 0));
}
