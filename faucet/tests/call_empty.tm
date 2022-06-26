# Create a proof of the admin badge
CALL_METHOD ComponentAddress("${account}") "create_proof" ResourceAddress("${admin_badge}");

# Call the restricted method
CALL_METHOD ComponentAddress("${component}") "empty";

# Stash away my bag
CALL_METHOD_WITH_ALL_RESOURCES ComponentAddress("${account}") "deposit_batch";
