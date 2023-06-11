use std::collections::HashMap;

#[derive(Debug)]
pub struct Balance {
    address: String,
    coins: Vec<Coin>,
}

#[derive(Debug)]
pub struct Coin {
    denom: String,
    amount: i128,
}

#[derive(Debug)]
pub struct MultiSend {
    inputs: Vec<Balance>,
    outputs: Vec<Balance>,
}

#[derive(Debug)]
struct DenomDefinition {
    denom: String,
    issuer: String,
    burn_rate: f64,
    commission_rate: f64,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_balance_changes() {
        // Test case setup
        let original_balances = vec![
            Balance {
                address: "account1".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: 1000 },
                    Coin { denom: "denom2".to_string(), amount: 2000 },
                ],
            },
            Balance {
                address: "account2".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: 500 },
                    Coin { denom: "denom2".to_string(), amount: 1500 },
                ],
            },
        ];

        let definitions = vec![
            DenomDefinition {
                denom: "denom1".to_string(),
                issuer: "issuer_account".to_string(),
                burn_rate: 0.1,
                commission_rate: 0.05,
            },
            DenomDefinition {
                denom: "denom2".to_string(),
                issuer: "issuer_account".to_string(),
                burn_rate: 0.2,
                commission_rate: 0.1,
            },
        ];

        let multi_send_tx = MultiSend {
            inputs: vec![
                Balance {
                    address: "account1".to_string(),
                    coins: vec![
                        Coin { denom: "denom1".to_string(), amount: 300 },
                        Coin { denom: "denom2".to_string(), amount: 1000 },
                    ],
                },
                Balance {
                    address: "account2".to_string(),
                    coins: vec![
                        Coin { denom: "denom1".to_string(), amount: 200 },
                        Coin { denom: "denom2".to_string(), amount: 500 },
                    ],
                },
            ],
            outputs: vec![
                Balance {
                    address: "account_recipient".to_string(),
                    coins: vec![
                        Coin { denom: "denom1".to_string(), amount: 500 },
                        Coin { denom: "denom2".to_string(), amount: 1500 },
                    ],
                },
                Balance {
                    address: "issuer_account".to_string(),
                    coins: vec![
                        Coin { denom: "denom1".to_string(), amount: 50 },
                        Coin { denom: "denom2".to_string(), amount: 100 },
                    ],
                },
            ],
        };

        // Expected balance changes
        let expected_balance_changes = vec![
            Balance {
                address: "account_recipient".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: 500 },
                    Coin { denom: "denom2".to_string(), amount: 1500 },
                ],
            },
            Balance {
                address: "issuer_account".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: 50 },
                    Coin { denom: "denom2".to_string(), amount: 100 },
                ],
            },
            Balance {
                address: "account1".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: -350 },
                    Coin { denom: "denom2".to_string(), amount: -1200 },
                ],
            },
            Balance {
                address: "account2".to_string(),
                coins: vec![
                    Coin { denom: "denom1".to_string(), amount: -150 },
                    Coin { denom: "denom2".to_string(), amount: -500 },
                ],
            },
        ];

        // Calculate balance changes
        let balance_changes = calculate_balance_changes(original_balances, definitions, multi_send_tx);

        // Compare with expected results
        assert_eq!(balance_changes, Ok(expected_balance_changes));
    }
}
