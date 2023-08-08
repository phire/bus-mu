use darling::{FromDeriveInput, FromVariant, ast::Data, util::{Ignored, PathList}};
use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[derive(Debug)]
#[derive(FromVariant)]
#[darling(attributes(named), and_then = "Self::validate")]
struct Variant {
    ident: syn::Ident,
    discriminant: Option<syn::Expr>,
    class: PathList,
}

impl Variant {
    fn validate(self) -> darling::Result<Self> {
        if self.discriminant.is_some() {
            return Err(darling::Error::custom("discriminants are not supported, enum must be continuous"));
        }
        match self.class.len() {
            1 => Ok(self),
            _ => Err(darling::Error::custom("expected exactly one class"))
        }
    }
}

#[derive(Debug)]
#[derive(FromDeriveInput)]
#[darling(attributes(named), supports(enum_unit))]
struct NamedEnum {
    data: Data<Variant, Ignored>,
    base: PathList,
    exit_reason: PathList,
}

#[proc_macro_derive(Named, attributes(named, exit_reason))]
pub fn derive_named(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input);
    let named_enum = NamedEnum::from_derive_input(&input).expect("failed to parse enum");
    let DeriveInput { ident, .. } = input;


    let storage_type = quote::format_ident!("{}Storage", ident);
    let storage_type_inner = quote::format_ident!("{}StorageInner", ident);
    let base = &named_enum.base[0];

    let mut from_patterns = Vec::new();
    let mut size_patterns = Vec::new();
    let mut storage_patterns = Vec::new();

    let mut classes = Vec::new();

    let mut varients = Vec::new();
    for variant in named_enum.data.take_enum().unwrap() {
        let class_path: &syn::Path = &variant.class[0];
        let name = variant.ident;

        let id = varients.len();
        varients.push(name.clone());

        from_patterns.push(quote! {
            #id => #class_path::name(),
        });
        size_patterns.push(quote! {
            #ident::#name => core::mem::size_of::<#class_path>(),
        });
        storage_patterns.push(quote! {
            #name: #base<#ident, #class_path>,
        });

        classes.push(quote! {
            impl actor_framework::Named<#ident> for #class_path {
                #[inline(always)] fn name() -> #ident { #ident::#name }
                #[inline(always)] fn dyn_name(&self) -> #ident { #ident::#name }
                #[inline(always)]
                fn from_storage<'a, 'b>(storage: &'a mut #storage_type)
                    -> &'b mut actor_framework::ActorBox<#ident, Self>
                where
                    'a: 'b
                {
                    &mut storage.inner.#name
                }
            }
        });
    }

    let count = varients.len();
    let exit_reason = &named_enum.exit_reason[0];

    let output = quote! {

        #(#classes)*

        impl actor_framework::MakeNamed for #ident {
            const COUNT: usize = #count;
            type Base<A> = #base<#ident, A> where A: actor_framework::Actor<#ident>;
            type ExitReason = Box<dyn #exit_reason>;
            type StorageType = #storage_type;
            type ArrayType<T> = [T; #count];

            fn size_of(id: Self) -> usize {
                match id {
                    #(#size_patterns)*
                }
            }

            fn index_array<T>(array: &Self::ArrayType<T>, id: Self) -> &T {
                &array[usize::from(id)]
            }
            fn index_array_mut<T>(array: &mut Self::ArrayType<T>, id: Self) -> &mut T {
                &mut array[usize::from(id)]
            }
            fn array_from_fn<T>(mut f: impl FnMut(Self) -> T) -> Self::ArrayType<T> {
                std::array::from_fn(|id| f(id.into()))
            }
        }

        struct #storage_type_inner {
            #(#storage_patterns)*
        }

        impl #storage_type_inner {
            fn base_ptr(&self, id: #ident) -> *mut actor_framework::ActorBoxBase<#ident> {
                unsafe {
                    match id {
                        #(#ident::#varients => std::mem::transmute(&self.#varients),)*
                    }
                }
            }
        }

        pub struct #storage_type {
            inner: Box<#storage_type_inner>,
            base_ptrs: [*mut actor_framework::ActorBoxBase<#ident>; #count]
        }

        impl actor_framework::AsBase<#ident> for #storage_type {
            #[inline(always)]
            fn as_base(&mut self, id: #ident) -> &mut actor_framework::ActorBoxBase<#ident> {
                // Soooo....
                // I was going to do this the safer way (see inner.base_ptr() above)
                // But rust/llvm generated really shitty code (using a jump table to do the job of an array)
                // So we get this this hack instead
                let i: usize = id.into();
                let ptr = self.base_ptrs[i];

                unsafe {
                    ptr.as_mut().unwrap_unchecked()
                }
            }
        }

        impl Default for #storage_type {
            fn default() -> Self {
                let inner = Box::new(#storage_type_inner {
                    #(#varients: Default::default(),)*
                });
                let base_ptrs = core::array::from_fn(|i| {
                    let id: #ident = i.into();
                    inner.base_ptr(id)
                });

                Self {
                    inner,
                    base_ptrs,
                }
            }
        }

        impl From<#ident> for usize {
            #[inline(always)]
            fn from(id: #ident) -> usize {
                id as usize
            }
        }

        impl From<usize> for #ident {
            #[inline(always)]
            fn from(id: usize) -> #ident {
                match id {
                    #(#from_patterns)*
                    _ => { panic!("invalid id"); }
                }
            }
        }
    };

    output.into()
}
