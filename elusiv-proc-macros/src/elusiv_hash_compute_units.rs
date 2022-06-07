use proc_macro2::TokenStream;
use quote::quote;
use super::utils::*;
use elusiv_computation::compute_unit_optimization;

const FULL_ROUNDS_CUS: u32 = 15411 + 17740 + 600;
const PARTIAL_ROUNDS_CUS: u32 = 5200 + 17740 + 600;

pub fn impl_elusiv_hash_compute_units(attrs: TokenStream) -> TokenStream {
    let attrs = sub_attrs_prepare(attrs.to_string());
    let attrs: Vec<&str> = (&attrs).split(",").collect();

    // Ident
    let id: TokenStream = attrs[0].parse().unwrap();

    // Number of hashes
    let hashes: usize = attrs[1].parse().unwrap();

    // Stub representation of our binary input Poseidon hash
    let mut rounds = Vec::new();
    for round in 0..65 * hashes {
        let round = round % 65;

        // Cost based on full or partial rounds
        rounds.push(
            if round < 4 || round >= 61 {   // 8 full rounds
                FULL_ROUNDS_CUS
            } else { // 57 partial rounds
                PARTIAL_ROUNDS_CUS
            }
        );
    }

    let result = compute_unit_optimization(rounds);

    let total_rounds = (hashes * 65) as u32;
    let total_compute_units = result.total_compute_units;
    assert_eq!(result.total_rounds, total_rounds);
    
    let size: TokenStream = result.instructions.len().to_string().parse().unwrap();
    let instructions = result.instructions.iter().fold(quote!{}, |acc, x| {
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