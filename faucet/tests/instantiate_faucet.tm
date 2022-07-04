CALL_METHOD ComponentAddress("${account}") "withdraw_by_amount" Decimal("${amount}") ResourceAddress("${asset}");
TAKE_FROM_WORKTOP_BY_AMOUNT Decimal("${amount}") ResourceAddress("${asset}") Bucket("bucket1");
CALL_FUNCTION PackageAddress("${package}") "Faucet" "instantiate_faucet" ${taker_badge} Bucket("bucket1") Decimal("${tap_amount}") ${taps_per_epoch} ${allow_empty_call} ${allow_stranger_fill};
CALL_METHOD_WITH_ALL_RESOURCES ComponentAddress("${account}") "deposit_batch";
