use super::grammar::*;
use super::storage::*;
use proc_macro2::{ TokenStream, Group, TokenTree };
use quote::quote;

pub fn interpret(computation: Vec<TokenTree>, name: &str, parameters: TokenStream, ty: TokenStream) -> TokenStream {
    let groups: Vec<Group> = computation.iter().map(|t| {
        if let TokenTree::Group(g) = t.clone() { g } else { panic!("Only scopes allowed at top level") }
    }).collect();
    let mut computation: Computation = (&groups[..]).into();

    // Find all variables and their declarations, usages, assignments
    let mut vars = Vars(vec![]);
    for (scope_index, scope) in computation.scopes.iter().enumerate() {
        // Declaration and assignment statements
        for stmt in scope.stmt.all_terminal_stmts() {
            match stmt {
                Stmt::Let(SingleId(id), _, Type(ty), _) => {
                    assert!(!vars.contains_var(&id), "Var '{}' cannot be redeclared", &id);
                    vars.0.push(Var::new(id.clone(), Some(ty.clone()), Some(scope_index)));
                },
                Stmt::Assign(SingleId(id), _) => {
                    match vars.get_var(&id) {
                        None => panic!("Assigned to var '{}' before declaring it", id),
                        Some(var) => Var::add(&mut var.assigns, scope_index)
                    }
                },
                _ => {}
            }
        }

        // Variable usages in all expressions
        let usages = scope.stmt.all_exprs().iter().map(|e| e.all_vars())
            .fold(vec![], |acc, v| acc.iter().cloned().chain(v.iter().cloned()).collect());
        
        for v in usages {
            match vars.get_var(&v) {
                Some(var) => Var::add(&mut var.usages, scope_index),
                None => {} // -> var does not exist -> caller has to provide it as parameter
            }
        }
    }

    // Add the storage read, write, free instructions
    for var in &vars.0 {
        let decl= var.declaration.clone().unwrap();
        if !var.used_outside_of_decl() { continue; }

        // Add write to declare scope
        computation.scopes[decl].write.push(MemoryId { id: var.id.clone(), ty: var.ty.clone().unwrap() });

        // Add write to assign scopes (when not the declare scope)
        for &assign in &var.assigns {
            if assign == decl { continue; }
            computation.scopes[assign].write.push(MemoryId { id: var.id.clone(), ty: var.ty.clone().unwrap() });
        }

        // Add read to usage scopes (when not the declare scope)
        for &usage in &var.usages {
            if usage == decl { continue; }
            // If there is an assignment in this scope, we use ReadMut
            computation.scopes[usage].read.push(MemoryRead {
                id: MemoryId { id: var.id.clone(), ty: var.ty.clone().unwrap() },
                mutable: matches!(var.assigns.iter().find(|&s| *s == usage), Some(_))
            });
        }

        // Add free after the last read
        if let Some(&last_usage) = var.usages.last() {
            computation.scopes[last_usage].free.push(MemoryId { id: var.id.clone(), ty: var.ty.clone().unwrap() });
        }
    }

    // Construct the match arms by iterating over all scopes
    let mut m = quote!{};
    let mut single_rounds: usize = 0;
    let mut multi_rounds = quote!{};
    let mut storage = StorageMappings { store: vec![] };
    for scope in computation.scopes { 
        let start_rounds = quote!{ #single_rounds #multi_rounds };
        let result = scope.stmt.to_stream(start_rounds.clone());
        let body = result.stream;

        let mut read = quote!{};
        let mut write = quote!{};
        let mut free = quote!{};

        for r in scope.read { read.extend(storage.read(r)); }
        for w in scope.write.clone() { write.extend(storage.write(w)); }

        let mut ram_in = quote!{};
        let mut ram_out = quote!{};
        // Partial computations require a RAM offset
        if body.to_string().contains("partial") {
            for m in &storage.store {
                let height = m.height();
                let name = ram_name(&m.ty);
                ram_in.extend(quote!{ #name.inc_frame(#height); });
                ram_out.extend(quote!{ #name.dec_frame(#height); });
            }
        }

        // If we free memory and write, we only free in the last iteration and write to different locations
        if !scope.free.is_empty() {
            let mut write_after_free = quote!{};
            for f in scope.free { free.extend(storage.free(f)); }
            for w in scope.write { write_after_free.extend(storage.write(w)); }

            if let Some(r) = result.rounds.clone() {
                write = quote!{
                    if round < #r - 1 {
                        #write
                    } else {
                        #write_after_free
                    }
                };
                free = quote!{ if round == #r - 1 { #free } };
            } else {
                write = write_after_free;
            }
        }

        match result.rounds {
            Some(r) => multi_rounds.extend(quote!{ + #r }),
            None => single_rounds += 1,
        }

        let round = if body.to_string().contains("round") {
            quote!{ let round = round - (#start_rounds); }
        } else { quote!{} };

        m.extend(quote!{
            round if round >= #start_rounds && round < #single_rounds #multi_rounds => {
                #round
                #read
                #free

                #ram_in #body #ram_out

                #write
            },
        });
    }

    let fn_name: TokenStream = format!("{}_partial", name).parse().unwrap();
    let rounds_count_name: TokenStream = format!("{}_ROUNDS_COUNT", name.to_uppercase()).parse().unwrap();

    // Check that all storage objects have been cleared (required to be able to move back to calling computation)
    for m in storage.store {
        assert_eq!(m.height(), 0, "Storage {} {:?} is not cleared before program exit!", m.ty, m.mapping.iter().filter_map(|x| x.clone()).collect::<Vec<String>>());
    }

    quote!{
        const #rounds_count_name: usize = #single_rounds #multi_rounds;
        pub fn #fn_name(round: usize, #parameters) -> Result<Option<#ty>, &'static str> {
            match round {
                #m
                _ => { }
            }
            Ok(None)
        }
    }
}

