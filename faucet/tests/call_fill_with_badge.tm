# Take required resources from the user
CALL_METHOD ComponentAddress("${account}") "withdraw_by_amount" Decimal("${amount}") ResourceAddress("${resource}");
TAKE_FROM_WORKTOP_BY_AMOUNT Decimal("${amount}") ResourceAddress("${resource}") Bucket("bucket1");

# Create a proof of the admin badge
CALL_METHOD ComponentAddress("${account}") "create_proof" ResourceAddress("${admin_badge}");

# Then put them into the faucet
CALL_METHOD ComponentAddress("${component}") "fill" Bucket("bucket1");

# Stash away my bag
#CALL_METHOD_WITH_ALL_RESOURCES ComponentAddress("${account}") "deposit_batch";
