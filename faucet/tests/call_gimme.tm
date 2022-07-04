# Create a proof of the taker badge
CALL_METHOD ComponentAddress("${account}") "create_proof" ResourceAddress("${taker_badge}");

# Call the restricted method
CALL_METHOD ComponentAddress("${component}") "gimme";

# Stash away my bag
CALL_METHOD_WITH_ALL_RESOURCES ComponentAddress("${account}") "deposit_batch";
