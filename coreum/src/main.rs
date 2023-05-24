use std::collections::HashMap;

fn main() {
    println!("Hello, Coreum!");
}

// A user can submit a `MultiSend` transaction (similar to bank.MultiSend in cosmos sdk) to transfer multiple
// coins (denoms) from multiple input addresses to multiple output addresses. A denom is the name or symbol
// for a coin type, e.g USDT and USDC can be considered different denoms; in cosmos ecosystem they are called
// denoms, in ethereum world they are called symbols.
// The sum of input coins and output coins must match for every transaction.
struct MultiSend {
    // inputs contain the list of accounts that want to send coins from, and how many coins from each account we want to send.
    inputs: Vec<Balance>,
    // outputs contains the list of accounts that we want to deposit coins into, and how many coins to deposit into
    // each account
    outputs: Vec<Balance>,
}

pub struct Coin {
    pub denom: String,
    pub amount: i128,
}

struct Balance {
    address: String,
    coins: Vec<Coin>,
}

// A Denom has a definition (`CoinDefinition`) which contains different attributes related to the denom:
struct DenomDefinition {
    // the unique identifier for the token (e.g `core`, `eth`, `usdt`, etc.)
    denom: String,
    // The address that created the token
    issuer: String,
    // burn_rate is a number between 0 and 1. If it is above zero, in every transfer,
    // some additional tokens will be burnt on top of the transferred value, from the senders address.
    // The tokens to be burnt are calculated by multiplying the TransferAmount by burn rate, and
    // rounding it up to an integer value. For example if an account sends 100 token and burn_rate is
    // 0.2, then 120 (100 + 100 * 0.2) will be deducted from sender account and 100 will be deposited to the recipient
    // account (i.e 20 tokens will be burnt)
    burn_rate: f64,
    // commission_rate is exactly same as the burn_rate, but the calculated value will be transferred to the
    // issuer's account address instead of being burnt.
    commission_rate: f64,
}

// Implement `calculate_balance_changes` with the following requirements.
// - Output of the function is the balance changes that must be applied to different accounts
//   (negative means deduction, positive means addition), or an error. the error indicates that the transaction must be rejected.
// - If sum of inputs and outputs in multi_send_tx does not match the tx must be rejected(i.e return error).
// - Apply burn_rate and commission_rate as described by their definition.
// - If the sender does not have enough balances (in the original_balances) to cover the input amount on top of burn_rate and
// commission_rate, the transaction must be rejected.
// - burn_rate and commission_rate does not apply to the issuer. So to calculate the correct values you must do this for every denom:
//      - sum all the inputs coming from accounts that are not an issuer (let's call it non_issuer_input_sum)
//      - sum all the outputs going to accounts that are not an issuer (let's call it non_issuer_output_sum)
//      - total burn amount is total_burn = min(non_issuer_input_sum, non_issuer_output_sum)
//      - total_burn is distributed between all input accounts as: account_share = roundup(total_burn * input_from_account / non_issuer_input_sum)
//      - total_burn_amount = sum (account_shares) // notice that in previous step we rounded up, so we need to recalculate the total again.
//      - commission_rate is exactly the same, but we send the calculate value to issuer, and not burn.
//      - Example:
//          burn_rate: 10%
//
//          inputs:
//          60, 90
//          25 <-- issuer
//
//          outputs:
//          50
//          100 <-- issuer
//          25
//          In this case burn amount is: min(non_issuer_inputs, non_issuer_outputs) = min(75+75, 50+25) = 75
//          Expected burn: 75 * 10% = 7.5
//          And now we divide it proportionally between all input sender: first_sender_share  = 7.5 * 60 / 150  = 3
//                                                                        second_sender_share = 7.5 * 90 / 150  = 4.5
// - In README.md we have provided more examples to help you better understand the requirements.
// - Write different unit tests to cover all the edge cases, we would like to see how you structure your tests.
//   There are examples in README.md, you can convert them into tests, but you should add more cases.

fn calculate_balance_changes(
    original_balances: Vec<Balance>,
    definitions: Vec<DenomDefinition>,
    multi_send_tx: MultiSend,
) -> Result<Vec<Balance>, String> {
    let mut balance_map: HashMap<String, HashMap<String, i128>> = HashMap::new();
    for balance in original_balances {
        let mut coin_map: HashMap<String, i128> = HashMap::new();
        for coin in balance.coins {
            coin_map.insert(coin.denom.clone(), coin.amount);
        }
        balance_map.insert(balance.address, coin_map);
    }

    let mut definition_map: HashMap<String, DenomDefinition> = HashMap::new();
    for definition in definitions {
        definition_map.insert(definition.denom.clone(), definition);
    }

    let mut input_total: HashMap<String, i128> = HashMap::new();
    let mut output_total: HashMap<String, i128> = HashMap::new();
    let mut non_issuer_input_sum: HashMap<String, i128> = HashMap::new();
    for balance in &multi_send_tx.inputs {
        for coin in &balance.coins {
            if balance_map.contains_key(&balance.address) {
                if let Some(balance_amount) = balance_map[&balance.address].get(&coin.denom) {
                    if balance_amount >= &coin.amount {
                        if let Some(definition) = definition_map.get(&coin.denom) {
                            if balance.address != definition.issuer {
                                let non_issuer_input = non_issuer_input_sum.entry(coin.denom.clone()).or_insert(0);
                                *non_issuer_input += coin.amount;
                                let total_input = input_total.entry(coin.denom.clone()).or_insert(0);
                                *total_input += coin.amount;
                            }
                        } else {
                            return Err(format!("Denomination {} does not have a definition", &coin.denom));
                        }
                    } else {
                        return Err(format!("{} does not have enough balance for {}", &balance.address, &coin.denom));
                    }
                }
            }
        }
    }
    for balance in &multi_send_tx.outputs {
        for coin in &balance.coins {
            let total_output = output_total.entry(coin.denom.clone()).or_insert(0);
            *total_output += coin.amount;
        }
    }
    for (denom, total_input) in &input_total {
        if output_total.get(denom).unwrap_or(&0) != total_input {
            return Err(format!("Input and output does not match for {}", denom));
        }
    }

    let mut balance_changes: Vec<Balance> = Vec::new();
    for balance in multi_send_tx.inputs {
        let mut new_coins: Vec<Coin> = Vec::new();
        for coin in balance.coins {
            if let Some(definition) = definition_map.get(&coin.denom) {
                if balance.address != definition.issuer {
                    let total_burn = (*non_issuer_input_sum.get(&coin.denom).unwrap()).min(*output_total.get(&coin.denom).unwrap()) as f64
                        * definition.burn_rate;
                    let account_share_burn = (total_burn
                        * coin.amount as f64
                        / *non_issuer_input_sum.get(&coin.denom).unwrap() as f64)
                        .ceil() as i128;
                    let total_burn_amount = account_share_burn;
                    let total_commission = (*non_issuer_input_sum.get(&coin.denom).unwrap()).min(*output_total.get(&coin.denom).unwrap()) as f64
                        * definition.commission_rate;
                    let account_share_commission = (total_commission
                        * coin.amount as f64
                        / *non_issuer_input_sum.get(&coin.denom).unwrap() as f64)
                        .ceil() as i128;
                    let total_commission_amount = account_share_commission;
                    let sender_balance = balance_map.get_mut(&balance.address).unwrap().get_mut(&coin.denom).unwrap();
                    *sender_balance -= coin.amount + total_burn_amount + total_commission_amount;
                    new_coins.push(Coin { denom: coin.denom.clone(), amount: -coin.amount - total_burn_amount - total_commission_amount });
                } else {
                    let sender_balance = balance_map.get_mut(&balance.address).unwrap().get_mut(&coin.denom).unwrap();
                    *sender_balance -= coin.amount;
                    new_coins.push(Coin { denom: coin.denom.clone(), amount: -coin.amount });
                }
            }
        }
        balance_changes.push(Balance { address: balance.address, coins: new_coins });
    }
    for balance in multi_send_tx.outputs {
        for coin in balance.coins {
            if let Some(receiver_balance) = balance_map.get_mut(&balance.address) {
                if let Some(receiver_coin_balance) = receiver_balance.get_mut(&coin.denom) {
                    *receiver_coin_balance += coin.amount;
                } else {
                    receiver_balance.insert(coin.denom.clone(), coin.amount);
                }
            } else {
                let mut new_balance: HashMap<String, i128> = HashMap::new();
                new_balance.insert(coin.denom.clone(), coin.amount);
                balance_map.insert(balance.address.clone(), new_balance);
            }
        }
    }
    Ok(balance_changes)
}


mod coreum {
    #[derive(Debug)]
    pub struct Balance {
        address: String,
        coins: Vec<Coin>,
    }

    #[derive(Debug)]
    pub struct Coin {
        denom: String,
        amount: u64,
    }
}


#[cfg(test)]
mod tests {
    use crate::{calculate_balance_changes, DenomDefinition, MultiSend};
    use super::coreum::{Balance, Coin};
    #[derive(Debug)]
    struct Balance {
        address: String,
        coins: Vec<Coin>,
    }

    #[derive(Debug)]
    struct Coin {
        denom: String,
        amount: u64,
    }

    #[derive(Debug)]
    struct BalanceChange {
        address: String,
        coins: Vec<Coin>,
    }

    #[test]
    fn test_balance_changes_no_issuer_on_sender_or_receiver() {
        let original_balances = vec![
            Balance {
                address: "account1".to_string(),
                coins: vec![Coin { denom: "denom1".to_string(), amount: 1000_000 }],
            },
            Balance {
                address: "account2".to_string(),
                coins: vec![Coin { denom: "denom2".to_string(), amount: 1000_000 }],
            }
        ];

        let definitions = vec![
            DenomDefinition {
                denom: "denom1".to_string(),
                issuer: "issuer_account_A".to_string(),
                burn_rate: 0.08,
                commission_rate: 0.12,
            },
            DenomDefinition {
                denom: "denom2".to_string(),
                issuer: "issuer_account_B".to_string(),
                burn_rate: 1.0,
                commission_rate: 0.0,
            },
        ];

        let multi_send = MultiSend {
            inputs: vec![
                Balance {
                    address: "account1".to_string(),
                    coins: vec![Coin { denom: "denom1".to_string(), amount: 1000 }],
                },
                Balance {
                    address: "account2".to_string(),
                    coins: vec![Coin { denom: "denom2".to_string(), amount: 1000 }],
                },
            ],
            outputs: vec![
                Balance {
                    address: "account_recipient".to_string(),
                    coins: vec![
                        Coin { denom: "denom1".to_string(), amount: 1000 },
                        Coin { denom: "denom2".to_string(), amount: 1000 },
                    ],
                },
            ],
        };

        let expected_balance_changes = vec![
            Balance {
                address: "account_recipient".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: 1000 },
                    Coin { denom: "denom2".to_string(), amount: 1000 },
                ],
            },
            Balance {
                address: "issuer_account_A".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: 120 },
                ],
            },
            Balance {
                address: "account1".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: -1200 },
                ],
            },
            Balance {
                address: "account2".to_string(),
                coins: vec![
                    Coin { denom: "denom2".to_string(), amount: -1000 },
                ],
            },
        ];

        let balance_changes = calculate_balance_changes(
            original_balances,
            definitions,
            multi_send,
        ).expect("Failed to calculate balance changes");

        assert_eq!(expected_balance_changes, balance_changes);
    }

    #[test]
    fn test_balance_changes_with_issuer_on_sender_or_receiver() {
        let original_balances: Vec<Balance> = vec![
            Balance {
                address: "address1".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: 1000 },
                ],
            },
            Balance {
                address: "address2".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: 1000 },
                ],
            },
            Balance {
                address: "issuer_address".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: 1000 },
                ],
            },
        ];

        let definitions: Vec<DenomDefinition> = vec![
            DenomDefinition {
                denom: "denom1".to_string(),
                issuer: "issuer_address".to_string(),
                burn_rate: 0.1,
                commission_rate: 0.05,
            },
        ];

        let multi_send: MultiSend = MultiSend {
            inputs: vec![
                Balance {
                    address: "address1".to_string(),
                    coins: vec![
                        Coin { denom: "denom1".to_string(), amount: 500 },
                    ],
                },
            ],
            outputs: vec![
                Balance {
                    address: "address2".to_string(),
                    coins: vec![
                        Coin { denom: "denom1".to_string(), amount: 425 },
                    ],
                },
            ],
        };

        let expected_balance_changes: Vec<BalanceChange> = vec![
            BalanceChange {
                address: "address1".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: -500 },
                ],
            },
            BalanceChange {
                address: "address2".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: 425 },
                ],
            },
            BalanceChange {
                address: "issuer_address".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: 75 },
                ],
            },
        ];

        let balance_changes = calculate_balance_changes(
            original_balances,
            definitions,
            multi_send,
        ).expect("Failed to calculate balance changes");

        assert_eq!(expected_balance_changes, balance_changes);
    }

    #[test]
    #[should_panic(expected = "Not enough balance.")]
    fn test_balance_changes_not_enough_balance() {
        let original_balances: Vec<Balance> = vec![
            Balance {
                address: "account1".to_string(),
                coins: vec![],
            },
        ];

        let definitions: Vec<DenomDefinition> = vec![
            DenomDefinition {
                denom: "denom1".to_string(),
                issuer: "issuer_account_A".to_string(),
                burn_rate: 0.0,
                commission_rate: 0.0,
            },
        ];

        let multi_send: MultiSend = MultiSend {
            inputs: vec![
                Balance {
                    address: "account1".to_string(),
                    coins: vec![Coin { denom: "denom1".to_string(), amount: 350 }],
                },
            ],
            outputs: vec![
                Balance {
                    address: "account_recipient".to_string(),
                    coins: vec![Coin { denom: "denom1".to_string(), amount: 350 }],
                },
            ],
        };

        calculate_balance_changes(original_balances, definitions, multi_send)
            .expect("Not enough balance.");
    }

    #[test]
    #[should_panic(expected = "Input and output mismatch.")]
    fn test_balance_changes_input_output_mismatch() {
        let original_balances: Vec<Balance> = vec![
            Balance {
                address: "account1".to_string(),
                coins: vec![Coin { denom: "denom1".to_string(), amount: 1000_000 }],
            },
        ];

        let definitions: Vec<DenomDefinition> = vec![
            DenomDefinition {
                denom: "denom1".to_string(),
                issuer: "issuer_account_A".to_string(),
                burn_rate: 0.0,
                commission_rate: 0.0,
            },
        ];

        let multi_send: MultiSend = MultiSend {
            inputs: vec![
                Balance {
                    address: "account1".to_string(),
                    coins: vec![Coin { denom: "denom1".to_string(), amount: 350 }],
                },
            ],
            outputs: vec![
                Balance {
                    address: "account_recipient".to_string(),
                    coins: vec![Coin { denom: "denom1".to_string(), amount: 450 }],
                },
            ],
        };

        calculate_balance_changes(original_balances, definitions, multi_send)
            .expect("Input and output mismatch.");
    }

    #[test]
    fn test_balance_changes_rounding_up() {
        let original_balances: Vec<Balance> = vec![
            Balance {
                address: "account1".to_string(),
                coins: vec![Coin { denom: "denom1".to_string(), amount: 1000 }],
            },
            Balance {
                address: "account2".to_string(),
                coins: vec![Coin { denom: "denom1".to_string(), amount: 1000 }],
            },
        ];

        let definitions: Vec<DenomDefinition> = vec![
            DenomDefinition {
                denom: "denom1".to_string(),
                issuer: "issuer_account_A".to_string(),
                burn_rate: 0.01,
                commission_rate: 0.01,
            },
        ];

        let multi_send: MultiSend = MultiSend {
            inputs: vec![
                Balance {
                    address: "account1".to_string(),
                    coins: vec![Coin { denom: "denom1".to_string(), amount: 1 }],
                },
                Balance {
                    address: "account2".to_string(),
                    coins: vec![Coin { denom: "denom1".to_string(), amount: 1 }],
                },
            ],
            outputs: vec![
                Balance {
                    address: "account_recipient".to_string(),
                    coins: vec![Coin { denom: "denom1".to_string(), amount: 2 }],
                },
            ],
        };

        let expected_balance_changes: Vec<BalanceChange> = vec![
            BalanceChange {
                address: "account_recipient".to_string(),
                coins: vec![Coin { denom: "denom1".to_string(), amount: 2 }],
            },
            BalanceChange {
                address: "issuer_account_A".to_string(),
                coins: vec![Coin { denom: "denom1".to_string(), amount: 2 }],
            },
            BalanceChange {
                address: "account1".to_string(),
                coins: vec![Coin { denom: "denom1".to_string(), amount: -3 }],
            },
            BalanceChange {
                address: "account2".to_string(),
                coins: vec![Coin { denom: "denom1".to_string(), amount: -3 }],
            },
        ];

        let balance_changes = calculate_balance_changes(
            original_balances,
            definitions,
            multi_send,
        ).expect("Failed to calculate balance changes");

        assert_eq!(expected_balance_changes, balance_changes);
    }
}
