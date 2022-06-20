import os

xrd = '030000000000000000000000000000000000000000000000000004'
account = '020d3869346218a5e8deaaf2001216dc00fcacb79fb43e30ded79a'
radiswap_package = '012d50808d1cd48385e29806b919842353ec30da1e62891a314d72'
basket_package = '013d376f655f8a8882a0c0ffb42de4e69d241a3782a875fc1cc58b'
basket = '0266ee6531c82cd259f903a38277df2897356e30be076dc3a219e3'
basket_admin_badge = '030a0fb7e5a32a2a89156b05588dcba0f0417fa237d3b74d462bfa'
basket_token = '037244e93bb7ee22d18558a46cb87a176404191dd61fc143a2aeb4'
basket_stake_receipt = '03b241b537e7dc7656c75864ab62f2274e1d50d8e156326a17d161'
samusky = '03edee7c05b983d9da72008cef885b72aaf64b7399bce4bea610e9'
token_A = '03b463e3cc20bb2fabeafa6312aa5e4e267b3bc49b05cb479f4cbb'
token_B = '03811108113188b70f526b012199ed7795375228390d25222a9b68'
token_C = '038028b8d0e72f68dfe978ddcfee55345ea4f3a3cab09586031ef8'
samusky_pool = '02758517381a0f82ffb5294bb04bf516bec40e3e622c1ac2d343c0'
pool_A = '02d0bc0a33a1eb9a53a3488a23acfb9abff601d0ff862467a922c7'
pool_B = '02abbff35739c613a3657340c398eb65b90fe201211ac603609178'
pool_C = '0255ad0df0f3a638ae3d7eaf3a7d36d264f7a07b70dcfa42066cd9'

def setup():
    os.system('resim reset')
    os.system('resim new-account')
    os.system('resim publish ./radiswap_modified')
    os.system('resim publish ./basket')
    os.system('resim new-token-fixed --name Samusky 1000000')
    os.system('resim new-token-fixed --name token_A 1000000')
    os.system('resim new-token-fixed --name token_B 3000000')
    os.system('resim new-token-fixed --name token_C 5000000')
    os.system('resim call-function {} Radiswap instantiate_pool {},{} {},{} {} {} {} {} {}'.format(radiswap_package, 100000, samusky, 10000, xrd, 100000, 'LPA', 'lp_A', 'www.Samusky.com', 0.01))
    os.system('resim call-function {} Radiswap instantiate_pool {},{} {},{} {} {} {} {} {}'.format(radiswap_package, 100000, token_A, 10000, xrd, 100000, 'LPA', 'lp_A', 'www.Acoin.com', 0.01))
    os.system('resim call-function {} Radiswap instantiate_pool {},{} {},{} {} {} {} {} {}'.format(radiswap_package, 300000, token_B, 10000, xrd, 100000, 'LPB', 'lp_B', 'www.Bcoin.com', 0.01))
    os.system('resim call-function {} Radiswap instantiate_pool {},{} {},{} {} {} {} {} {}'.format(radiswap_package, 500000, token_C, 10000, xrd, 100000, 'LPC', 'lp_C', 'www.Ccoin.com', 0.01))
    os.system('resim call-function {} Basket instantiate_fund {} {} {} {}'.format(basket_package, samusky, samusky_pool, 10, 2))


def show(address):
    os.system('resim show {}'.format(address))

def add_investment(pool):
    with open('temp.rtm', 'w') as f:
        f.write('CALL_METHOD ComponentAddress("{}") "create_proof" ResourceAddress("{}");'.format(account, basket_admin_badge))
        f.write('CALL_METHOD ComponentAddress("{}") "add_investment" ComponentAddress("{}");'.format(basket, pool))

    os.system('resim run temp.rtm')
    os.remove('temp.rtm')

def set_radix_reserve_percent(reserve_percent):
    with open('temp.rtm', 'w') as f:
        f.write('CALL_METHOD ComponentAddress("{}") "create_proof" ResourceAddress("{}");'.format(account, basket_admin_badge))
        f.write('CALL_METHOD ComponentAddress("{}") "set_radix_reserve_percent" Decimal("{}");'.format(basket, reserve_percent))

    os.system('resim run temp.rtm')
    os.remove('temp.rtm')
    
def set_fee_percent(fee_percent):
    with open('temp.rtm', 'w') as f:
        f.write('CALL_METHOD ComponentAddress("{}") "create_proof" ResourceAddress("{}");'.format(account, basket_admin_badge))
        f.write('CALL_METHOD ComponentAddress("{}") "set_fee_percent" Decimal("{}");'.format(basket, fee_percent))

    os.system('resim run temp.rtm')
    os.remove('temp.rtm')

def buy(amount):
    os.system('resim call-method {} buy {},{}'.format(basket, amount, xrd))

def sell(amount):
    os.system('resim call-method {} sell {},{}'.format(basket, amount, basket_token))

def stake(amount, pool):
    os.system('resim call-method {} stake {},{} {}'.format(basket, amount, samusky, pool))

def unstake(id):
    os.system('resim call-method {} unstake #{},{}'.format(basket, id, basket_stake_receipt))

def cancel_staking(id):
    os.system('resim call-method {} cancel_staking #{},{}'.format(basket, id, basket_stake_receipt))

def collect_unstaked(id):
    os.system('resim call-method {} collect_unstaked #{},{}'.format(basket, id, basket_stake_receipt))

def rebalance():
    os.system('resim call-method {} rebalance'.format(basket))

def get_prices():
    os.system('resim call-method {} get_prices'.format(basket))

def get_amounts():
    os.system('resim call-method {} get_amounts'.format(basket))

def get_mean_adjusted_changes():
    os.system('resim call-method {} get_mean_adjusted_changes'.format(basket))

def get_total_value():
    os.system('resim call-method {} get_total_value'.format(basket))

def get_value():
    os.system('resim call-method {} get_value'.format(basket))

def get_total_stake():
    os.system('resim call-method {} get_total_stake'.format(basket))

def get_stake_denominator():
    os.system('resim call-method {} get_stake_denominator'.format(basket))

def amm_buy(amount, pool):
    os.system('resim call-method {} swap {},{}'.format(pool, amount, xrd))

if __name__ == '__main__':
    setup()
    add_investment(pool_A)
    add_investment(pool_B)
    add_investment(pool_C)
    set_fee_percent(1)
    set_radix_reserve_percent(15)
    buy(1000)
    stake(30, pool_A)
    stake(20, pool_A)
    stake(10, pool_A)
    stake(20, pool_B)
    stake(10, pool_C)
    rebalance()
    buy(1000)
    amm_buy(1000, pool_B)
    rebalance()
    unstake('ef9e8b8eecb92a06241732ec794d000f')
    rebalance()
    collect_unstaked('ef9e8b8eecb92a06241732ec794d000f')
    stake(10, pool_B)
    cancel_staking('eb863dafe12375d58d4302a7a9aa1bc4')
    show(account)
    buy(100)
    sell(78.492141234840935831)

    show(basket)
    show(account)