use super::utils::*;
use elusiv_computation::{compute_unit_optimization, MAX_COMPUTE_UNIT_LIMIT};
use elusiv_proc_macro_utils::try_parse_usize;
use proc_macro2::TokenStream;
use quote::quote;

const COMPUTE_UNIT_PADDING: u32 = 20_000;
const FULL_ROUNDS_CUS: u32 = 15411 + 17740 + 600;
const PARTIAL_ROUNDS_CUS: u32 = 5200 + 17740 + 600;

pub fn impl_elusiv_hash_compute_units(attrs: TokenStream) -> TokenStream {
    let attrs = sub_attrs_prepare(attrs.to_string());
    let attrs: Vec<&str> = attrs.split(',').collect();

    // Ident
    let id: TokenStream = attrs[0].parse().unwrap();

    // Number of hashes
    let hashes: usize = attrs[1].parse().unwrap();

    // Optional compute units reduction
    let reduction: Option<u32> = if let Some(attr) = attrs.get(2) {
        try_parse_usize(attr).map(|v| v as u32)
    } else {
        None
    };

    // Stub representation of our binary input Poseidon hash
    let mut rounds = Vec::new();
    for round in 0..65 * hashes {
        let round = round % 65;

        // Cost based on full or partial rounds
        rounds.push(if !(4..61).contains(&round) {
            // 8 full rounds
            FULL_ROUNDS_CUS
        } else {
            // 57 partial rounds
            PARTIAL_ROUNDS_CUS
        });
    }

    let max_compute_budget = MAX_COMPUTE_UNIT_LIMIT - COMPUTE_UNIT_PADDING - reduction.unwrap_or(0);
    let result = compute_unit_optimization(rounds, max_compute_budget);

    let total_rounds = (hashes * 65) as u32;
    let total_compute_units = result.total_compute_units;
    assert_eq!(result.total_rounds, total_rounds);

    let size: TokenStream = result.instructions.len().to_string().parse().unwrap();
    let instructions = result.instructions.iter().fold(quote! {}, |acc, &rounds| {
        assert!(rounds <= u8::MAX as u32);
        let rounds: TokenStream = rounds.to_string().parse().unwrap();
        quote! { #acc #rounds, }
    });
    let max_cus = MAX_COMPUTE_UNIT_LIMIT;

    quote! {
        impl elusiv_computation::PartialComputation<#size> for #id {
            const TX_COUNT: usize = #size;
            const INSTRUCTION_ROUNDS: [u8; #size] = [ #instructions ];
            const TOTAL_ROUNDS: u32 = #total_rounds;
            const TOTAL_COMPUTE_UNITS: u32 = #total_compute_units;
            const COMPUTE_BUDGET_PER_IX: u32 = #max_cus;
        }
    }
}
