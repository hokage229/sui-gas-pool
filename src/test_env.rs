// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::config::GasStationConfig;
use crate::gas_pool_initializer::GasPoolInitializer;
use crate::gas_station::gas_station_core::GasStationContainer;
use std::sync::Arc;
use sui_swarm_config::genesis_config::AccountConfig;
use sui_types::base_types::{ObjectRef, SuiAddress};
use sui_types::crypto::get_account_key_pair;
use sui_types::gas_coin::MIST_PER_SUI;
use sui_types::signature::GenericSignature;
use sui_types::transaction::{TransactionData, TransactionDataAPI};
use test_cluster::{TestCluster, TestClusterBuilder};

pub async fn start_sui_cluster(init_gas_amounts: Vec<u64>) -> (TestCluster, GasStationConfig) {
    std::env::set_var("RPC_QUERY_MAX_RESULT_LIMIT", "3000");
    let (sponsor, keypair) = get_account_key_pair();
    let cluster = TestClusterBuilder::new()
        .with_accounts(vec![
            AccountConfig {
                address: Some(sponsor),
                gas_amounts: init_gas_amounts,
            },
            // Besides sponsor, also initialize another account with 10 SUI.
            AccountConfig {
                address: None,
                gas_amounts: vec![MIST_PER_SUI; 10],
            },
        ])
        .build()
        .await;
    let fullnode_url = cluster.fullnode_handle.rpc_url.clone();
    let config = GasStationConfig {
        keypair: keypair.into(),
        fullnode_url,
        ..Default::default()
    };
    (cluster, config)
}

pub async fn start_gas_station(init_gas_amounts: Vec<u64>) -> (TestCluster, GasStationContainer) {
    let (test_cluster, config) = start_sui_cluster(init_gas_amounts).await;
    let GasStationConfig {
        keypair,
        gas_pool_config,
        fullnode_url,
        ..
    } = config;
    let keypair = Arc::new(keypair);
    let storage = GasPoolInitializer::run(
        fullnode_url.as_str(),
        &gas_pool_config,
        MIST_PER_SUI,
        keypair.clone(),
    )
    .await;
    let station = GasStationContainer::new(keypair, storage, fullnode_url.as_str()).await;
    (test_cluster, station)
}

pub async fn create_test_transaction(
    test_cluster: &TestCluster,
    sponsor: SuiAddress,
    gas_coins: Vec<ObjectRef>,
) -> (TransactionData, GenericSignature) {
    let user = test_cluster
        .get_addresses()
        .into_iter()
        .find(|a| *a != sponsor)
        .unwrap();
    let object = test_cluster
        .wallet
        .get_one_gas_object_owned_by_address(user)
        .await
        .unwrap()
        .unwrap();
    let mut tx_data = test_cluster
        .test_transaction_builder_with_gas_object(user, gas_coins[0])
        .await
        .transfer(object, user)
        .build();
    // TODO: Add proper sponsored transaction support to test tx builder.
    tx_data.gas_data_mut().payment = gas_coins;
    tx_data.gas_data_mut().owner = sponsor;
    let user_sig = test_cluster
        .sign_transaction(&tx_data)
        .into_data()
        .tx_signatures_mut_for_testing()
        .pop()
        .unwrap();
    (tx_data, user_sig)
}
