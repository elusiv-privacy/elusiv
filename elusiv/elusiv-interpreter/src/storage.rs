use proc_macro2::TokenStream;
use quote::quote;

#[derive(Debug, Clone)]
pub struct MemoryId {
    pub id: String,
    pub ty: String,
}

#[derive(Debug, Clone)]
pub struct MemoryRead {
    pub id: MemoryId,
    pub mutable: bool,
}

pub struct Vars(pub Vec<Var>);
impl Vars {
    pub fn contains_var(&self, id: &str) -> bool {
        matches!(self.0.iter().find(|var| var.id == id), Some(_))
    }

    pub fn get_var(&mut self, id: &str) -> Option<&mut Var> {
        let pos = self.0.iter().position(|var| var.id == id);
        match pos {
            Some(p) => Some(&mut self.0[p]),
            _ => None,
        }
    }
}

pub struct Var {
    pub id: String,
    pub ty: Option<String>,

    pub declaration: Option<usize>,
    pub usages: Vec<usize>,
    pub assigns: Vec<usize>,
}

impl Var {
    pub fn new(id: String, ty: Option<String>, declaration: Option<usize>) -> Self {
        Var {
            id,
            ty,
            declaration,
            usages: vec![],
            assigns: vec![],
        }
    }

    pub fn add(scopes: &mut Vec<usize>, scope: usize) {
        if matches!(scopes.iter().position(|&s| s == scope), None) {
            scopes.push(scope);
        }
    }

    // Returns true if the var is being used in a different scope than the one it was declared in
    pub fn used_outside_of_decl(&self) -> bool {
        let decl = self.declaration.unwrap();
        matches!(self.usages.iter().find(|&u| *u != decl), Some(_))
    }
}

pub struct StorageMappings {
    pub store: Vec<StorageMapping>,
}

/// Maps variable names to indices
#[derive(Clone)]
pub struct StorageMapping {
    /// Mapping: if the var `a` is stored at the first index, mapping[0] == Some("a")
    pub mapping: Vec<Option<String>>,
    /// Type of the storage mapping (all types in a single storage manager need to be homogenous)
    pub ty: String,
}

impl StorageMappings {
    pub fn read(&mut self, r: MemoryRead) -> TokenStream {
        let m = self.get_mapping(&r.id.ty);
        if !m.contains(&r.id.id) {
            m.allocate(&r.id.id);
        }

        let index = m.get_position(&r.id.id);
        let name = ram_name(&r.id.ty);
        let id = &r.id.id.parse::<TokenStream>().unwrap();

        if r.mutable {
            quote! { let mut #id = #name.read(#index); }
        } else {
            quote! { let #id = #name.read(#index); }
        }
    }

    pub fn free(&mut self, r: MemoryId) {
        let m = self.get_mapping(&r.ty);
        if !m.contains(&r.id) {
            m.allocate(&r.id);
        }

        m.deallocate(&r.id);
    }

    pub fn write(&mut self, w: MemoryId) -> TokenStream {
        let m = self.get_mapping(&w.ty);
        if !m.contains(&w.id) {
            m.allocate(&w.id);
        }

        let index = m.get_position(&w.id);
        let name = ram_name(&w.ty);
        let id = &w.id.parse::<TokenStream>().unwrap();

        // if there is a lower spot, than index, use that
        let first = m.first_free();
        if first < index {
            m.deallocate(&w.id);
            m.allocate(&w.id);
            quote! {
                #name.write(#id, #first);
            }
        } else {
            quote! { #name.write(#id, #index); }
        }
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

impl StorageMapping {
    pub fn new(size: usize, ty: String) -> Self {
        StorageMapping {
            mapping: vec![None; size],
            ty,
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        let r = self.mapping.iter().find(|x| match x {
            None => false,
            Some(x) => x == id,
        });
        matches!(r, Some(_))
    }

    fn get_position(&self, id: &str) -> usize {
        self.mapping
            .iter()
            .position(|x| match x {
                None => false,
                Some(x) => x == id,
            })
            .unwrap()
    }

    fn first_free(&self) -> usize {
        for (i, m) in self.mapping.iter().enumerate() {
            match m {
                None => {
                    return i;
                }
                Some(_) => {}
            }
        }
        panic!("No space left for allocation")
    }

    pub fn height(&self) -> usize {
        for i in 0..self.mapping.len() {
            let index = self.mapping.len() - 1 - i;
            if self.mapping[index].is_some() {
                return index + 1;
            }
        }
        0
    }

    pub fn allocate(&mut self, id: &str) {
        if self.contains(id) {
            panic!("Cannot allocate var '{}' twice", id)
        }
        let index = self.first_free();
        self.mapping[index] = Some(String::from(id));
    }

    pub fn deallocate(&mut self, id: &str) {
        if !self.contains(id) {
            panic!("Cannot deallocate var '{}'", id)
        }
        let index = self.get_position(id);
        self.mapping[index] = None;
    }
}

pub fn ram_name(ty: &str) -> TokenStream {
    format!("storage.ram_{}", ty.to_lowercase())
        .parse::<TokenStream>()
        .unwrap()
}
