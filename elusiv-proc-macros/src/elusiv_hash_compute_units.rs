use proc_macro2::TokenStream;
use quote::quote;
use super::utils::*;
use elusiv_computation::{
    PartialComputationInstruction,
    COMPUTE_UNIT_PADDING,
    MAX_COMPUTE_UNIT_LIMIT,
};

const MAX_CUS: u32 = MAX_COMPUTE_UNIT_LIMIT - COMPUTE_UNIT_PADDING;

const FULL_ROUNDS_CUS: u32 = 33_000;
const PARTIAL_ROUNDS_CUS: u32 = 22_800;

pub fn impl_elusiv_hash_compute_units(attrs: TokenStream) -> TokenStream {
    let attrs = sub_attrs_prepare(attrs.to_string());
    let attrs: Vec<&str> = (&attrs).split(",").collect();

    // Ident
    let id = attrs[0];
    let const_id: TokenStream = format!("{}_INSTRUCTIONS", id.to_uppercase()).parse().unwrap();

    // Number of hashes
    let hashes: usize = attrs[1].parse().unwrap();

    let mut instructions = Vec::new();

    let mut rounds = 0;
    let mut compute_units = 0;

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
            instructions.push(PartialComputationInstruction { rounds, compute_units });

            rounds = 0;
            compute_units = 0;
        } else {
            rounds += 1;
            compute_units += next_cost;
        }
    }

    if rounds > 0 {
        instructions.push(PartialComputationInstruction { rounds, compute_units });
    }

    let size: TokenStream = instructions.len().to_string().parse().unwrap();
    let instructions = instructions.iter().fold(quote!{}, |acc, x| {
        let rounds = x.rounds;
        let compute_units = x.compute_units;

        quote! {
            #acc
            elusiv_computation::PartialComputationInstruction { rounds: #rounds, compute_units: #compute_units },
        }
    });

    quote! {
        pub const #const_id: [elusiv_computation::PartialComputationInstruction; #size] = [
            #instructions
        ];
    }
}