from cmath import polar
from multiprocessing import pool
import os

xrd = '030000000000000000000000000000000000000000000000000004'
account = '020d3869346218a5e8deaaf2001216dc00fcacb79fb43e30ded79a'
radiswap_package = '0163c801dc8e1b0f16a27dcb198348ce6e962aee82a6ae3b6aa8b7'
basket_package = '017cc75d5322923a3a15105d65296bf329158f674d8e79f83c1a06'
basket = '02c908a49b37bcb655f194f9cc9e51d63a8c6b4706bd10964fff03'
basket_admin_badge = '038f7810182c8067ba0c9fb7a1d05025e84700094e34c513834af0'
basket_token = '030ffa4a94d5f6a375501b40f2e537f54f0a2c36391cc0f75ed6cb'
basket_stake_receipt = '03d7e73032779c0998c1dd4a565337ced3c920c450e27bc0fca249'
samusky = '03edee7c05b983d9da72008cef885b72aaf64b7399bce4bea610e9'
token_A = '03b463e3cc20bb2fabeafa6312aa5e4e267b3bc49b05cb479f4cbb'
token_B = '03811108113188b70f526b012199ed7795375228390d25222a9b68'
token_C = '038028b8d0e72f68dfe978ddcfee55345ea4f3a3cab09586031ef8'
samusky_pool = '029f3b668d79b431c4c4b6952cdb8ef32b734c9771e29a7522e007'
pool_A = '0292d8a1eeb89f7c210597ea7fd2cabde3f1dffb8ae759c8fa85f6'
pool_B = '024cb1d3befe8df4f1a5056332d7f871d93afa64fab8ae47cc5cb9'
pool_C = '02d8539e3dec7f47e3aa7b1f3f9903a1fcb519620871345fc4d95c'
lp_token_A = '03b7b7a1ae62f4fc3df798d2b6e31d2245665d92b39ea0156541ae'
lp_token_B = '03fa987636ac885d8d75290d911c32b395cdec88a74b149da69aa0'
lp_token_C = '039c779fe3738008c69ba497680eb6946df77b9c1d793791de1d13'

def setup():
    os.system('resim reset')
    os.system('resim new-account')
    os.system('resim publish ./radiswap_modified')
    os.system('resim publish ./basket')
    os.system('resim new-token-fixed --name Samusky 1000000')
    os.system('resim new-token-fixed --name token_A 1000000')
    os.system('resim new-token-fixed --name token_B 3000000')
    os.system('resim new-token-fixed --name token_C 5000000')
    os.system('resim call-function {} Radiswap instantiate_pool {},{} {},{} {} {} {} {} {}'.format(radiswap_package, 100000, samusky, 10000, xrd, 100000, 'LPS', 'lp_S', 'www.Samusky.com', 0.01))
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

def amm_remove_liquidity(amount, lp_token, pool):
    os.system('resim call-method {} remove_liquidity {},{}'.format(pool, amount, lp_token))

if __name__ == '__main__':
    setup()
    add_investment(pool_A)
    add_investment(pool_B)
    add_investment(pool_C)
    stake(30, pool_A)
    stake(20, pool_B)
    stake(10, pool_C)
    rebalance()
    buy(100)
    show(basket)
    show(account)