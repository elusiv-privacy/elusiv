use proc_macro2::TokenStream;
use quote::quote;
use super::utils::*;
use elusiv_computation::{
    PartialComputationInstruction,
    COMPUTE_UNIT_PADDING,
    MAX_COMPUTE_UNIT_LIMIT,
};

const MAX_CUS: u32 = MAX_COMPUTE_UNIT_LIMIT - COMPUTE_UNIT_PADDING;

const FULL_ROUNDS_CUS: u32 = 15411 + 17740 + 600;
const PARTIAL_ROUNDS_CUS: u32 = 5200 + 17740 + 600;

pub fn impl_elusiv_hash_compute_units(attrs: TokenStream) -> TokenStream {
    let attrs = sub_attrs_prepare(attrs.to_string());
    let attrs: Vec<&str> = (&attrs).split(",").collect();

    // Ident
    let id: TokenStream = attrs[0].parse().unwrap();

    // Number of hashes
    let hashes: usize = attrs[1].parse().unwrap();

    let mut instructions = Vec::new();

    let mut rounds = 0;
    let mut start_round = 0;
    let mut compute_units = 0;
    let mut total_compute_units = 0;

    // Stub representation of our binary input Poseidon hash
    for round in 0..65 * hashes {
        let round = round % 65;

        // Cost based on full or partial rounds
        let next_cost = if round < 4 || round >= 61 {   // 8 full rounds
            FULL_ROUNDS_CUS
        } else { // 57 partial rounds
            PARTIAL_ROUNDS_CUS
        };

        if compute_units + next_cost > MAX_CUS {
            instructions.push(PartialComputationInstruction { start_round, rounds, compute_units: compute_units + COMPUTE_UNIT_PADDING });

            start_round += rounds;
            rounds = 1;
            compute_units = next_cost;
        } else {
            rounds += 1;
            compute_units += next_cost;
        }

        total_compute_units += next_cost;
    }

    if rounds > 0 {
        instructions.push(PartialComputationInstruction { start_round, rounds, compute_units: compute_units + COMPUTE_UNIT_PADDING });
    }


    let total_rounds = (hashes * 65) as u32;
    assert!(start_round + rounds == total_rounds);

    let size: TokenStream = instructions.len().to_string().parse().unwrap();
    let instructions = instructions.iter().fold(quote!{}, |acc, x| {
        let start_round = x.start_round;
        let rounds = x.rounds;
        let compute_units = x.compute_units;

        quote! {
            #acc
            elusiv_computation::PartialComputationInstruction {
                start_round: #start_round,
                rounds: #rounds,
                compute_units: #compute_units,
            },
        }
    });

    quote! {
        pub struct #id { }

        impl elusiv_computation::PartialComputation<#size> for #id {
            const INSTRUCTIONS: [elusiv_computation::PartialComputationInstruction; #size] = [
                #instructions
            ];
            const TOTAL_ROUNDS: u32 = #total_rounds;
            const TOTAL_COMPUTE_UNITS: u32 = #total_compute_units;
        }
    }
}