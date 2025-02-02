// Copyright 2019-2021 Parity Technologies (UK) Ltd.
// This file is part of Parity Bridges Common.

// Parity Bridges Common is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity Bridges Common is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity Bridges Common.  If not, see <http://www.gnu.org/licenses/>.

//! A mock runtime for testing different stuff in the crate. We've been using Millau
//! runtime for that before, but it has two drawbacks:
//!
//! - circular dependencies between this crate and Millau runtime;
//!
//! - we can't use (e.g. as git subtree or by copying) this crate in repo without Millau.

#![cfg(test)]

use crate::messages::{
	source::{
		FromThisChainMaximalOutboundPayloadSize, FromThisChainMessagePayload,
		FromThisChainMessageVerifier, TargetHeaderChainAdapter,
	},
	target::{FromBridgedChainMessagePayload, SourceHeaderChainAdapter},
	BridgedChainWithMessages, HashOf, MessageBridge, ThisChainWithMessages,
};

use bp_header_chain::HeaderChain;
use bp_messages::{target_chain::ForbidInboundMessages, LaneId, MessageNonce};
use bp_parachains::SingleParaStoredHeaderDataBuilder;
use bp_runtime::{Chain, ChainId, Parachain, UnderlyingChainProvider};
use codec::{Decode, Encode};
use frame_support::{
	parameter_types,
	weights::{ConstantMultiplier, IdentityFee, RuntimeDbWeight, Weight},
};
use pallet_transaction_payment::Multiplier;
use sp_runtime::{
	testing::H256,
	traits::{BlakeTwo256, ConstU32, ConstU64, ConstU8, IdentityLookup},
	FixedPointNumber, Perquintill,
};

/// Account identifier at `ThisChain`.
pub type ThisChainAccountId = u64;
/// Balance at `ThisChain`.
pub type ThisChainBalance = u64;
/// Block number at `ThisChain`.
pub type ThisChainBlockNumber = u32;
/// Hash at `ThisChain`.
pub type ThisChainHash = H256;
/// Hasher at `ThisChain`.
pub type ThisChainHasher = BlakeTwo256;
/// Runtime call at `ThisChain`.
pub type ThisChainRuntimeCall = RuntimeCall;
/// Runtime call origin at `ThisChain`.
pub type ThisChainCallOrigin = RuntimeOrigin;
/// Header of `ThisChain`.
pub type ThisChainHeader = sp_runtime::generic::Header<ThisChainBlockNumber, ThisChainHasher>;
/// Block of `ThisChain`.
pub type ThisChainBlock = frame_system::mocking::MockBlock<TestRuntime>;
/// Unchecked extrinsic of `ThisChain`.
pub type ThisChainUncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;

/// Account identifier at the `BridgedChain`.
pub type BridgedChainAccountId = u128;
/// Balance at the `BridgedChain`.
pub type BridgedChainBalance = u128;
/// Block number at the `BridgedChain`.
pub type BridgedChainBlockNumber = u32;
/// Hash at the `BridgedChain`.
pub type BridgedChainHash = H256;
/// Hasher at the `BridgedChain`.
pub type BridgedChainHasher = BlakeTwo256;
/// Header of the `BridgedChain`.
pub type BridgedChainHeader =
	sp_runtime::generic::Header<BridgedChainBlockNumber, BridgedChainHasher>;

/// Message lane used in tests.
pub const TEST_LANE_ID: LaneId = LaneId([0, 0, 0, 0]);
/// Maximal number of queued messages at the test lane.
pub const MAXIMAL_PENDING_MESSAGES_AT_TEST_LANE: MessageNonce = 32;
/// Minimal extrinsic weight at the `BridgedChain`.
pub const BRIDGED_CHAIN_MIN_EXTRINSIC_WEIGHT: usize = 5;
/// Maximal extrinsic weight at the `BridgedChain`.
pub const BRIDGED_CHAIN_MAX_EXTRINSIC_WEIGHT: usize = 2048;
/// Maximal extrinsic size at the `BridgedChain`.
pub const BRIDGED_CHAIN_MAX_EXTRINSIC_SIZE: u32 = 1024;

frame_support::construct_runtime! {
	pub enum TestRuntime where
		Block = ThisChainBlock,
		NodeBlock = ThisChainBlock,
		UncheckedExtrinsic = ThisChainUncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Utility: pallet_utility,
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>},
		BridgeRelayers: pallet_bridge_relayers::{Pallet, Call, Storage, Event<T>},
		BridgeGrandpa: pallet_bridge_grandpa::{Pallet, Call, Storage},
		BridgeParachains: pallet_bridge_parachains::{Pallet, Call, Storage, Event<T>},
		BridgeMessages: pallet_bridge_messages::{Pallet, Call, Storage, Event<T>, Config<T>},
	}
}

crate::generate_bridge_reject_obsolete_headers_and_messages! {
	ThisChainRuntimeCall, ThisChainAccountId,
	BridgeGrandpa, BridgeParachains, BridgeMessages
}

parameter_types! {
	pub const ActiveOutboundLanes: &'static [LaneId] = &[TEST_LANE_ID];
	pub const BridgedChainId: ChainId = *b"brdg";
	pub const BridgedParasPalletName: &'static str = "Paras";
	pub const ExistentialDeposit: ThisChainBalance = 500;
	pub const DbWeight: RuntimeDbWeight = RuntimeDbWeight { read: 1, write: 2 };
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	pub const TransactionBaseFee: ThisChainBalance = 0;
	pub const TransactionByteFee: ThisChainBalance = 1;
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(3, 100_000);
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000u128);
	pub MaximumMultiplier: Multiplier = sp_runtime::traits::Bounded::max_value();
}

impl frame_system::Config for TestRuntime {
	type RuntimeOrigin = RuntimeOrigin;
	type Index = u64;
	type RuntimeCall = RuntimeCall;
	type BlockNumber = ThisChainBlockNumber;
	type Hash = ThisChainHash;
	type Hashing = ThisChainHasher;
	type AccountId = ThisChainAccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = ThisChainHeader;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU32<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<ThisChainBalance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = frame_support::traits::Everything;
	type SystemWeightInfo = ();
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = DbWeight;
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_utility::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = ();
}

impl pallet_balances::Config for TestRuntime {
	type Balance = ThisChainBalance;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
}

impl pallet_transaction_payment::Config for TestRuntime {
	type OnChargeTransaction = pallet_transaction_payment::CurrencyAdapter<Balances, ()>;
	type OperationalFeeMultiplier = ConstU8<5>;
	type WeightToFee = IdentityFee<ThisChainBalance>;
	type LengthToFee = ConstantMultiplier<ThisChainBalance, TransactionByteFee>;
	type FeeMultiplierUpdate = pallet_transaction_payment::TargetedFeeAdjustment<
		TestRuntime,
		TargetBlockFullness,
		AdjustmentVariable,
		MinimumMultiplier,
		MaximumMultiplier,
	>;
	type RuntimeEvent = RuntimeEvent;
}

impl pallet_bridge_grandpa::Config for TestRuntime {
	type BridgedChain = BridgedUnderlyingChain;
	type MaxRequests = ConstU32<50>;
	type HeadersToKeep = ConstU32<8>;
	type MaxBridgedAuthorities = ConstU32<1024>;
	type WeightInfo = pallet_bridge_grandpa::weights::BridgeWeight<TestRuntime>;
}

impl pallet_bridge_parachains::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type BridgesGrandpaPalletInstance = ();
	type ParasPalletName = BridgedParasPalletName;
	type ParaStoredHeaderDataBuilder =
		SingleParaStoredHeaderDataBuilder<BridgedUnderlyingParachain>;
	type HeadsToKeep = ConstU32<8>;
	type MaxParaHeadDataSize = ConstU32<1024>;
	type WeightInfo = pallet_bridge_parachains::weights::BridgeWeight<TestRuntime>;
}

impl pallet_bridge_messages::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_bridge_messages::weights::BridgeWeight<TestRuntime>;
	type ActiveOutboundLanes = ActiveOutboundLanes;
	type MaxUnrewardedRelayerEntriesAtInboundLane = ConstU64<16>;
	type MaxUnconfirmedMessagesAtInboundLane = ConstU64<16>;

	type MaximalOutboundPayloadSize = FromThisChainMaximalOutboundPayloadSize<OnThisChainBridge>;
	type OutboundPayload = FromThisChainMessagePayload;

	type InboundPayload = FromBridgedChainMessagePayload<ThisChainRuntimeCall>;
	type InboundRelayer = BridgedChainAccountId;
	type DeliveryPayments = ();

	type TargetHeaderChain = TargetHeaderChainAdapter<OnThisChainBridge>;
	type LaneMessageVerifier = FromThisChainMessageVerifier<OnThisChainBridge>;
	type DeliveryConfirmationPayments = pallet_bridge_relayers::DeliveryConfirmationPaymentsAdapter<
		TestRuntime,
		frame_support::traits::ConstU64<100_000>,
		frame_support::traits::ConstU64<10_000>,
	>;

	type SourceHeaderChain = SourceHeaderChainAdapter<OnThisChainBridge>;
	type MessageDispatch =
		ForbidInboundMessages<(), FromBridgedChainMessagePayload<ThisChainRuntimeCall>>;
	type BridgedChainId = BridgedChainId;
}

impl pallet_bridge_relayers::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type Reward = ThisChainBalance;
	type PaymentProcedure = ();
	type WeightInfo = ();
}

/// Bridge that is deployed on `ThisChain` and allows sending/receiving messages to/from
/// `BridgedChain`.
#[derive(Debug, PartialEq, Eq)]
pub struct OnThisChainBridge;

impl MessageBridge for OnThisChainBridge {
	const THIS_CHAIN_ID: ChainId = *b"this";
	const BRIDGED_CHAIN_ID: ChainId = *b"brdg";
	const BRIDGED_MESSAGES_PALLET_NAME: &'static str = "";

	type ThisChain = ThisChain;
	type BridgedChain = BridgedChain;
	type BridgedHeaderChain = pallet_bridge_grandpa::GrandpaChainHeaders<TestRuntime, ()>;
}

/// Bridge that is deployed on `BridgedChain` and allows sending/receiving messages to/from
/// `ThisChain`.
#[derive(Debug, PartialEq, Eq)]
pub struct OnBridgedChainBridge;

impl MessageBridge for OnBridgedChainBridge {
	const THIS_CHAIN_ID: ChainId = *b"brdg";
	const BRIDGED_CHAIN_ID: ChainId = *b"this";
	const BRIDGED_MESSAGES_PALLET_NAME: &'static str = "";

	type ThisChain = BridgedChain;
	type BridgedChain = ThisChain;
	type BridgedHeaderChain = ThisHeaderChain;
}

/// Dummy implementation of `HeaderChain` for `ThisChain` at the `BridgedChain`.
pub struct ThisHeaderChain;

impl HeaderChain<ThisUnderlyingChain> for ThisHeaderChain {
	fn finalized_header_state_root(_hash: HashOf<ThisChain>) -> Option<HashOf<ThisChain>> {
		unreachable!()
	}
}

/// Call origin at `BridgedChain`.
#[derive(Clone, Debug)]
pub struct BridgedChainOrigin;

impl From<BridgedChainOrigin>
	for Result<frame_system::RawOrigin<BridgedChainAccountId>, BridgedChainOrigin>
{
	fn from(
		_origin: BridgedChainOrigin,
	) -> Result<frame_system::RawOrigin<BridgedChainAccountId>, BridgedChainOrigin> {
		unreachable!()
	}
}

/// Underlying chain of `ThisChain`.
pub struct ThisUnderlyingChain;

impl Chain for ThisUnderlyingChain {
	type BlockNumber = ThisChainBlockNumber;
	type Hash = ThisChainHash;
	type Hasher = ThisChainHasher;
	type Header = ThisChainHeader;
	type AccountId = ThisChainAccountId;
	type Balance = ThisChainBalance;
	type Index = u32;
	type Signature = sp_runtime::MultiSignature;

	fn max_extrinsic_size() -> u32 {
		BRIDGED_CHAIN_MAX_EXTRINSIC_SIZE
	}

	fn max_extrinsic_weight() -> Weight {
		Weight::zero()
	}
}

/// The chain where we are in tests.
pub struct ThisChain;

impl UnderlyingChainProvider for ThisChain {
	type Chain = ThisUnderlyingChain;
}

impl ThisChainWithMessages for ThisChain {
	type RuntimeOrigin = ThisChainCallOrigin;
	type RuntimeCall = ThisChainRuntimeCall;

	fn is_message_accepted(_send_origin: &Self::RuntimeOrigin, lane: &LaneId) -> bool {
		*lane == TEST_LANE_ID
	}

	fn maximal_pending_messages_at_outbound_lane() -> MessageNonce {
		MAXIMAL_PENDING_MESSAGES_AT_TEST_LANE
	}
}

impl BridgedChainWithMessages for ThisChain {
	fn verify_dispatch_weight(_message_payload: &[u8]) -> bool {
		unreachable!()
	}
}

/// Underlying chain of `BridgedChain`.
pub struct BridgedUnderlyingChain;
/// Some parachain under `BridgedChain` consensus.
pub struct BridgedUnderlyingParachain;
/// Runtime call of the `BridgedChain`.
#[derive(Decode, Encode)]
pub struct BridgedChainCall;

impl Chain for BridgedUnderlyingChain {
	type BlockNumber = BridgedChainBlockNumber;
	type Hash = BridgedChainHash;
	type Hasher = BridgedChainHasher;
	type Header = BridgedChainHeader;
	type AccountId = BridgedChainAccountId;
	type Balance = BridgedChainBalance;
	type Index = u32;
	type Signature = sp_runtime::MultiSignature;

	fn max_extrinsic_size() -> u32 {
		BRIDGED_CHAIN_MAX_EXTRINSIC_SIZE
	}
	fn max_extrinsic_weight() -> Weight {
		Weight::zero()
	}
}

impl Chain for BridgedUnderlyingParachain {
	type BlockNumber = BridgedChainBlockNumber;
	type Hash = BridgedChainHash;
	type Hasher = BridgedChainHasher;
	type Header = BridgedChainHeader;
	type AccountId = BridgedChainAccountId;
	type Balance = BridgedChainBalance;
	type Index = u32;
	type Signature = sp_runtime::MultiSignature;

	fn max_extrinsic_size() -> u32 {
		BRIDGED_CHAIN_MAX_EXTRINSIC_SIZE
	}
	fn max_extrinsic_weight() -> Weight {
		Weight::zero()
	}
}

impl Parachain for BridgedUnderlyingParachain {
	const PARACHAIN_ID: u32 = 42;
}

/// The other, bridged chain, used in tests.
pub struct BridgedChain;

impl UnderlyingChainProvider for BridgedChain {
	type Chain = BridgedUnderlyingChain;
}

impl ThisChainWithMessages for BridgedChain {
	type RuntimeOrigin = BridgedChainOrigin;
	type RuntimeCall = BridgedChainCall;

	fn is_message_accepted(_send_origin: &Self::RuntimeOrigin, _lane: &LaneId) -> bool {
		unreachable!()
	}

	fn maximal_pending_messages_at_outbound_lane() -> MessageNonce {
		unreachable!()
	}
}

impl BridgedChainWithMessages for BridgedChain {
	fn verify_dispatch_weight(message_payload: &[u8]) -> bool {
		message_payload.len() >= BRIDGED_CHAIN_MIN_EXTRINSIC_WEIGHT &&
			message_payload.len() <= BRIDGED_CHAIN_MAX_EXTRINSIC_WEIGHT
	}
}
