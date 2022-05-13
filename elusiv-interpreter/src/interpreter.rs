use std::ops::RangeBounds;

use super::grammar::*;
use proc_macro2::{ TokenStream, Group, TokenTree };
use quote::quote;

/*
- nested partial computations

- compute units
- compute units sub-calls

- interpreter documentation and comments

- instructions and accounts
- pda renting system
- add doc/comments to other macros

- poseidon constants generator

- unary operators
*/

pub fn interpret(computation: Vec<TokenTree>, name: &str, parameters: TokenStream, ty: TokenStream) -> TokenStream {
    let groups: Vec<Group> = computation.iter().map(|t| {
        if let TokenTree::Group(g) = t.clone() { g } else { panic!("Only scopes allowed at top level") }
    }).collect();

    let mut computation: Computation = (&groups[..]).into();

    // Loop over all scopes
    let mut m = quote!{};
    let mut single_rounds: usize = 0;
    let mut multi_rounds = quote!{};
    for scope in computation.scopes { 
        let start_rounds = quote!{ #single_rounds #multi_rounds };
        let result = scope.stmt.to_stream(start_rounds.clone());
        let body = result.stream;

        match result.rounds {
            Some(r) => multi_rounds.extend(quote!{ + #r }),
            None => single_rounds += 1,
        }

        m.extend(quote!{
            round if round >= #start_rounds && round < #single_rounds #multi_rounds => {
                #body
            },
        });
    }

    let fn_name: TokenStream = format!("{}_partial", name).parse().unwrap();
    let rounds_count_name: TokenStream = format!("{}_ROUNDS_COUNT", name.to_uppercase()).parse().unwrap();

    // Find all variables
    /*let mut vars = Vars(vec![]);
    let mut parameter_vars = Vars(vec![]);
    let mut stored_types = vec![];
    for (scope_index, scope) in computation.scopes.iter().enumerate() {
        for stmt in scope.stmts.clone() {
            let stmts = stmt.get_all_stmts();
            for stmt in stmts {
                match stmt.clone() {
                    Stmt::Let(id, ty, _, _) => {
                        // Check that var has not been declared before
                        if vars.contains_var(&id.get_main_var()) {
                            panic!("Var '{}' cannot be redeclared", &id.get_main_var())
                        }

                        // Check that var is not a parameter var already
                        if parameter_vars.contains_var(&id.get_main_var()) {
                            panic!("Parameter-var '{}' cannot be redeclared", &id.get_main_var())
                        }

                        vars.0.push(Var::new(id.get_main_var().clone(), Some(ty.0), Some(scope_index)));
                    },
                    Stmt::Assign(id, _) => {
                        // Check that var has been declared before
                        match vars.get_var(&id.get_main_var()) {
                            Some(var) => { Var::add(&mut var.assigns, scope_index); },
                            None => panic!("Assigned to var '{}' before declaring it", id.get_main_var())
                        }
                    },
                    _ => {}
                }

                // Variable usages in expressions
                let stmt_expr_vars: Vec<String> = stmt.get_all_exprs().iter()
                    .map(|e| e.get_used_vars())
                    .fold(vec![], |acc, v| acc.iter().cloned().chain(v.iter().cloned()).collect());
                
                for v in stmt_expr_vars {
                    match vars.get_var(&v) {
                        Some(var) => { Var::add(&mut var.usages, scope_index); },
                        None => {   // Since the var has not been declared, it's considered a parameter var
                            parameter_vars.0.push(Var::new(v, None, Some(scope_index))); 
                        }
                    }
                }
            }
        }
    }

    // Add variable lifetime/scoping stmts
    for var in &vars.0 {
        let decl_scope = var.declaration.clone().unwrap();

        // Add write stmt to declare scope
        if var.used_outside_of_decl() {
            computation.scopes[decl_scope].write.push(MemoryWrite{ id: var.id.clone(), ty: var.ty.clone().unwrap() });
        }

        // Add write stmts to assign scopes (when != declare scope)
        for &assign in &var.assigns {
            if assign == decl_scope { continue; }
            computation.scopes[assign].write.push(MemoryWrite{ id: var.id.clone(), ty: var.ty.clone().unwrap() });
        }
        
        // Add read stmts to usage scopes (when != declare scope)
        for &usage in &var.usages {
            if usage == decl_scope { continue; }

            stored_types.push(var.ty.clone().unwrap());

            // If there is an assignment in this scope, we use ReadMut
            if matches!(var.assigns.iter().find(|&s| *s == usage), Some(_)) {
                computation.scopes[usage].read.push(MemoryRead { id: var.id.clone(), kind: MemoryReadKind::ReadMut, ty: var.ty.clone().unwrap() });
            } else {
                computation.scopes[usage].read.push(MemoryRead { id: var.id.clone(), kind: MemoryReadKind::Read, ty: var.ty.clone().unwrap() });
            }
        }

        // Add free stmt after the last read
        if let Some(&last_usage) = var.usages.last() {
            if last_usage == decl_scope { continue; }
            computation.scopes[last_usage].read.push(MemoryRead { id: var.id.clone(), kind: MemoryReadKind::Free, ty: var.ty.clone().unwrap() });
        }
    }

    /*
   
    let round = n - (100 * 3 + 100 * 2);

    if round >= 0 && round < 100 * 3 {
        let round = round - 0;
        if round < 100 * 3 - 1 {

        } else {
            let round = n...
        }
    }
    
    if round >= 100 * 3 && round < 100 * 3 + 100 * 2 {
        let round = round - 100 * 3;
        if round < 100 * 2 - 1 {

        } else {

        }
    }
    
    */

    // Generate the actual computation logic combined with computing the RAM indices
    let mut m = quote!{};
    let mut offsets = quote!{};
    let mut storage = StorageMappings { store: vec![] };
    let mut return_type = quote!{ () };
    let mut sub = 0;
    for (i, scope) in computation.scopes.iter().enumerate() {
        let k = i - sub;
        let rounds = quote!{ #k #offsets };
        let mut read = quote!{};
        let mut free = quote!{};
        let mut write = quote!{};

        for r in &scope.read {
            read.extend(storage.read(r.clone()));
            free.extend(storage.free(r.clone()));
        }

        for w in &scope.write { write.extend(storage.write(w.clone())) }

        if let Stmt::Let(Id::Single(vid), Type(_), LetKind::Partial, expr) = &scope.stmts.first().unwrap() {
            if scope.stmts.len() > 1 { panic!("Partial computations are only allowed in isolated scopes") }
            if let Expr::Fn(Id::Single(fid), exprs) = expr {
                let params: TokenStream = exprs.iter().fold(String::new(), |acc, x| format!("{}, {}", acc, x.to_string())).parse().unwrap();
                let var: TokenStream = vid.parse().unwrap();
                let fname: TokenStream = format!("{}_partial", fid).parse().unwrap();
                let partial_rounds_count: TokenStream = format!("{}_ROUNDS_COUNT", fid.to_uppercase()).parse().unwrap();

                sub += 1;
                offsets.extend(quote!{ + #partial_rounds_count });

                let mut ram_in = quote!{};
                let mut ram_out = quote!{};
                for ram in &storage.store {
                    let offset = ram.height();
                    let name = ram_name(&ram.ty);
                    ram_in.extend(quote!{ #name.inc_frame(#offset); });
                    ram_out.extend(quote!{ #name.dec_frame(#offset); });
                }

                m.extend(quote!{
                    n if n >= #rounds && n < #rounds + #partial_rounds_count => {
                        #read
                        #ram_in
                        let round = n - (#rounds);
                        if round < #partial_rounds_count - 1 {
                            match #fname(round #params) {
                                Ok(_) => {},
                                _ => return Err("Computation error")
                            }
                            #ram_out
                        } else {
                            let #var = match #fname(round #params) {
                                Ok(Some(v)) => v,
                                _ => return Err("Computation error")
                            };
                            #ram_out
                            #free
                            #write
                        }
                    },
                });
            } else { panic!("Invalid partial stmt") }
        } else if let Stmt::For(id, expr, stmt) = &scope.stmts.first().unwrap() {
            if let Expr::Array(arr) = expr {
                if let Id::Single(id) = id {
                    let count = arr.len();
                    let c = stmt.to_string().parse::<TokenStream>().unwrap();
                    let id: TokenStream = id.parse().unwrap();
                    let v: TokenStream = expr.to_string().parse().unwrap();

                    sub += 1;
                    offsets.extend(quote!{ + #count });

                    m.extend(quote!{
                        n if n >= #rounds && n < #rounds + #count => {
                            #read
                            let round = n - (#rounds);
                            let v = vec!#v;
                            let #id = v[round];
                            #c
                            if n == #rounds + #count - 1 { #free }
                            #write
                        },
                    });
                }
            } 
        } else {
            let mut c = quote!{};
            for (j, stmt) in scope.stmts.iter().enumerate() {
                // Return statement
                if let Stmt::Return(Expr::Id(id)) = stmt {
                    assert!(j == scope.stmts.len() - 1 && i == computation.scopes.len() - 1, "Only the last statement can be a return statement");
                    return_type = vars.get_var(&id.get_main_var()).unwrap().ty.clone().unwrap().parse().unwrap();
                }

                c.extend(stmt.to_string().parse::<TokenStream>().unwrap());
            }

            m.extend(quote!{ n if n == #rounds => { #read #free #c #write }, });
        }
    }
    let count = computation.scopes.len() - sub;
    let rounds_count = quote!{ #count #offsets };

    // Check that all storage objects have been cleared (required to be able to move back to calling computation)
    for m in storage.store {
        if m.height() != 0 {
            let vars: Vec<String> = m.mapping.iter().filter_map(|v| v.clone()).collect();
            panic!("Storage {} {:?} is not cleared before program exit!", m.ty, vars);
        }
    }

    // Parameters
    // - all vars that have not been declared are considered parameters
    // - for each stored var type there has to be a fitting RAM paramter
    let mut ram_params = quote!{};
    stored_types.sort();
    stored_types.dedup();
    for ty in stored_types {
        let name = ram_name(&ty);
        let ty = ty.parse::<TokenStream>().unwrap();
        ram_params.extend(quote!{ #name: &mut RAM<#ty>, });
    }
    
    let fn_name: TokenStream = format!("{}_partial", name).parse().unwrap();
    let rounds_count_name: TokenStream = format!("{}_ROUNDS_COUNT", name.to_uppercase()).parse().unwrap();

    quote!{
        const #rounds_count_name: usize = #rounds_count;

        pub fn #fn_name(round: usize, #ram_params #parameters) -> Result<Option<#return_type>, &'static str> {
            match round { #m _ => {} }
            Ok(None)
        }
    }*/
    quote!{
        pub fn #fn_name(round: usize) -> Result<Option<()>, &'static str> {
            match round {
                #m
                _ => { }
            }
            Ok(None)
        }
    }
}

struct Vars(pub Vec<Var>);
impl Vars {
    pub fn contains_var(&self, id: &str) -> bool {
        matches!(self.0.iter().find(|var| var.id == id), Some(_))
    }

    pub fn get_var(&mut self, id: &str) -> Option<&mut Var> {
        let pos = self.0.iter().position(|var| var.id == id);
        match pos { Some(p) => Some(&mut self.0[p]), _ => None }
    }
}

struct Var {
    pub id: String,
    pub ty: Option<String>,

    pub declaration: Option<usize>,
    pub usages: Vec<usize>,
    pub assigns: Vec<usize>,
}

impl Var {
    pub fn new(id: String, ty: Option<String>, declaration: Option<usize>) -> Self {
        Var { id, ty, declaration, usages: vec![], assigns: vec![] }
    }

    pub fn add(scopes: &mut Vec<usize>, scope: usize) {
        if matches!(scopes.iter().position(|&s| s == scope), None) {
            scopes.push(scope);
        }
    }

    // Returns true if the var is being used in a different scope than the one it was declared in
    pub fn used_outside_of_decl(&self) -> bool {
        let decl = self.declaration.clone().unwrap();
        matches!(self.usages.iter().find(|&u| *u != decl), Some(_))
    }
}

struct StorageMappings {
    pub store: Vec<StorageMapping>,
}

impl StorageMappings {
    pub fn read(&mut self, r: MemoryRead) -> TokenStream {
        let m = self.get_mapping(&r.ty);
        if !m.contains(&r.id) { m.allocate(&r.id); }

        let index = m.get_position(&r.id);
        let name = ram_name(&r.ty);
        let id = &r.id.parse::<TokenStream>().unwrap();
        match r.kind {
            MemoryReadKind::Read => quote!{ let #id = #name.read(#index); },
            MemoryReadKind::ReadMut => quote!{ let mut #id = #name.read(#index); },
            _ => { quote!{} }
        }
    }

    pub fn free(&mut self, r: MemoryRead) -> TokenStream {
        let m = self.get_mapping(&r.ty);
        if !m.contains(&r.id) { m.allocate(&r.id); }

        let index = m.get_position(&r.id);
        let name = ram_name(&r.ty);
        let id = &r.id.parse::<TokenStream>().unwrap();
        match r.kind {
            MemoryReadKind::Free => {
                m.deallocate(&r.id);
                quote!{ #name.free(#index); }
            },
            _ => { quote!{} }
        }
    }

    pub fn write(&mut self, w: MemoryWrite) -> TokenStream {
        let m = self.get_mapping(&w.ty);
        if !m.contains(&w.id) { m.allocate(&w.id); }

        let index = m.get_position(&w.id);
        let name = ram_name(&w.ty);
        let id = &w.id.parse::<TokenStream>().unwrap();
        quote!{ #name.write(#id, #index); }
    }

    fn get_mapping(&mut self, ty: &str) -> &mut StorageMapping {
        if let Some(i) = self.store.iter().position(|m| m.ty == ty) {
            &mut self.store[i]
        } else {
            let m = StorageMapping::new(100, String::from(ty));
            self.store.push(m);
            let i = self.store.len() - 1;
            &mut self.store[i]
        }
    }
}

#[derive(Clone)]
struct StorageMapping {
    mapping: Vec<Option<String>>,
    ty: String,
}

impl StorageMapping {
    pub fn new(size: usize, ty: String) -> Self {
        StorageMapping { mapping: vec![None; size], ty }
    }

    pub fn contains(&self, id: &str) -> bool {
        let r = self.mapping.iter().find(|x| match x { None => false, Some(x) => x == id });
        matches!(r, Some(_))
    }

    fn get_position(&self, id: &str) -> usize {
        self.mapping.iter().position(|x| match x { None => false, Some(x) => x == id }).unwrap()
    }

    fn first_free(&self) -> usize {
        for (i, m) in self.mapping.iter().enumerate() {
            match m { None => { return i; }, Some(_) => { } }
        }
        panic!("No space left for allocation")
    }

    fn height(&self) -> usize {
        for i in 0..self.mapping.len() {
            let index = self.mapping.len() - 1 - i;
            if let Some(_) = self.mapping[index] { return index + 1 }
        }
        0
    }

    pub fn allocate(&mut self, id: &str) {
        if self.contains(id) { panic!("Cannot allocate var '{}' twice", id) }
        let index = self.first_free();
        self.mapping[index] = Some(String::from(id));
    }

    pub fn deallocate(&mut self, id: &str) {
        if !self.contains(id) { panic!("Cannot deallocate var '{}'", id) }
        let index = self.get_position(id);
        self.mapping[index] = None;
    }
}

fn ram_name(ty: &str) -> TokenStream {
    format!("ram_{}", ty.to_lowercase()).parse::<TokenStream>().unwrap()
}