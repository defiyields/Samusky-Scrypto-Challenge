DexC2C
======================
DexC2C is a decentralized RADIX token otc trading service that allows you to create buy or sell orders that can be shared on social networks to match counterparties, or to specify and send to a counterparty to complete a transaction.

## Features

  * Create an intention to buy or sell
  * Seller escrow funds security, transparent and visible to buyers
  * Dispute over purchase and sale, admin access to confirm share ratio
  * The transaction process is not dependent on third parties and is guaranteed by contract

## Ticket

  To ensure that the contract is not abused, the creation or acceptance of an intent requires the user to first pledge XRD to obtain a ticket. After the transaction is completed, the pledged XRD can be redeemed by refund_ticket(). To simplify the process workflow does not reflect the ticket part of the process


## Work flow

![workflow](res/biz_flow.jpg)


## Build & Run (with window powershell)

* Environment preparation and reset.
* Create 3 accounts

| name    | usage                                   |
| ------- | --------------------------------------- |
| admin   | Used to deploy the contract.            |
| p1      | buyer     |
| p2      | seller                       |

```shell
scrypto build
resim reset
$result=resim publish .
$package=$result.substring("Success! New Package: ".length,$result.length-"Success! New Package: ".length)

$result=resim new-account
$admin=($result[1]).substring("Account component address: ".length,($result[1]).length-"Account component address: ".length)
$admin_priv=($result[3]).substring("Private key: ".length,($result[3]).length-"Private key: ".length)

$result=resim new-account
$p1=($result[1]).substring("Account component address: ".length,($result[1]).length-"Account component address: ".length)
$p1_priv=($result[3]).substring("Private key: ".length,($result[3]).length-"Private key: ".length)

$result=resim new-account
$p2=($result[1]).substring("Account component address: ".length,($result[1]).length-"Account component address: ".length)
$p2_priv=($result[3]).substring("Private key: ".length,($result[3]).length-"Private key: ".length)
$xrd='030000000000000000000000000000000000000000000000000004'
```

* Init component
  
```shell
resim set-default-account $admin $admin_priv
$result=resim call-function $package DexC2C new 100
$component=($result[10]).substring('鈹斺攢 Component: '.length, ($result[10]).length-'鈹斺攢 Component: '.length)
$ticket=($result[13]).substring('鈹斺攢 Resource: '.length, ($result[13]).length-'鈹斺攢 Resource: '.length)
```

* An example of a complete process including arbitration
  
```shell 
1: buyer buy ticket
    resim set-default-account $p1 $p1_priv
    resim call-method $component buy_ticket 100,$xrd
    resim show $p1

2: seller buy ticket
    resim set-default-account $p2 $p2_priv
    resim call-method $component buy_ticket 100,$xrd
    resim show $p2

3: seller ask
    resim set-default-account $p2 $p2_priv
    resim call-method $component ask $p1_id $p2_id $xrd 10 'Ask dispute test' '#0000000000000002',$ticket 
    resim call-method $component get_pending_order_by_id 1

4: buyer accept
    resim set-default-account $p1 $p1_priv
    resim call-method $component accept 1 '#0000000000000001',$ticket
    resim call-method $component get_pending_order_by_id 1

5: seller escrow
    resim set-default-account $p2 $p2_priv
    resim call-method $component seller_escrow 1 10,$xrd '#0000000000000002',$ticket
    resim call-method $component get_pending_order_by_id 1

6: buyer paid
    resim set-default-account $p1 $p1_priv
    resim call-method $component buyer_paid 1 '#0000000000000001',$ticket
    resim call-method $component get_pending_order_by_id 1
    
7: admin resolve dispute
    resim set-default-account $admin $admin_priv
    resim run resolve_dispute.txt (Note that replacing the variable)

8: buyer withdraw
    resim set-default-account $p1 $p1_priv
    resim call-method $component buyer_withdraw 1 '#0000000000000001',$ticket
    resim show $p1

9: seller withdraw
    resim set-default-account $p2 $p2_priv
    resim call-method $component seller_withdraw 1 '#0000000000000002',$ticket
    resim show $p2
```