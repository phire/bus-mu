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

    let mut make_patterns = Vec::new();
    let mut from_patterns = Vec::new();

    let mut classes = Vec::new();
    for variant in named_enum.data.take_enum().unwrap() {
        let class_path: &syn::Path = &variant.class[0];
        let name = variant.ident;
        let id = make_patterns.len();

        make_patterns.push(quote! {
            #ident::#name => Box::new(#class_path::default()),
        });
        from_patterns.push(quote! {
            #id => #class_path::name(),
        });

        classes.push(quote! {
            impl actor_framework::Named<#ident> for #class_path {
                #[inline(always)] fn name() -> #ident { #ident::#name }
                #[inline(always)] fn dyn_name(&self) -> #ident { #ident::#name }
            }
        });
    }

    let count = make_patterns.len();
    let base = &named_enum.base[0];
    let exit_reason = &named_enum.exit_reason[0];

    let output = quote! {

        #(#classes)*

        impl actor_framework::MakeNamed for #ident {
            const COUNT: usize = #count;
            type Base = dyn #base<#ident>;
            type ExitReason = Box<dyn #exit_reason>;

            fn make(id: Self) -> Box<Self::Base> {
                match id {
                    #(#make_patterns)*
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
